//! Commands that change the project (an op, a plan, undo, redo, save) and `history`. Each opens the
//! project, changes it in memory and writes it back, except under `--dry-run`.

use std::path::Path;

use dc_core::{Change, Command, History, Project, TimeMap};
use dc_ops::{Applied, Registry, Step, duration_change};
use dc_project::history_log::Event;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::find::find_project;
use crate::globals::Globals;
use crate::run::Answer;
use crate::workspace::Workspace;

const DRY_RUN: &str = "dry-run, проект не изменён: ";

pub(crate) struct Ctx<'a> {
    pub globals: &'a Globals,
    pub cwd: &'a Path,
    pub registry: &'a Registry,
}

impl Ctx<'_> {
    pub(crate) fn open(&self, write: bool) -> Result<Workspace, CliError> {
        let dir = find_project(self.globals.project.as_deref(), self.cwd)?;
        Workspace::open(dir, write && !self.globals.dry_run, self.globals.dry_run)
    }

    fn prefix(&self) -> &'static str {
        if self.globals.dry_run { DRY_RUN } else { "" }
    }
}

pub(crate) fn answer(workspace: &mut Workspace, text: String, json: Value) -> Answer {
    Answer {
        text,
        json,
        warnings: std::mem::take(&mut workspace.warnings),
    }
}

/// `devcast <op> [args]`: one op as one undo step.
pub(crate) fn op(ctx: &Ctx<'_>, words: &[String]) -> Result<Answer, CliError> {
    let (info, taken) = ctx.registry.resolve(words)?;
    let args = if info.name == "cut" {
        crate::transcript::word_ranges(ctx, &words[taken..])?
    } else {
        words[taken..].to_vec()
    };
    let fields = dc_ops::parse_args(info, &args)?;
    apply_steps(ctx, &[Step::new(info.name, fields)])
}

/// `devcast apply plan.json`: all steps of the plan as one undo step.
pub(crate) fn plan(ctx: &Ctx<'_>, path: &Path) -> Result<Answer, CliError> {
    let path = ctx.cwd.join(path);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        CliError::Failed(format!(
            "не удалось прочитать план «{}» ({e}): проверь путь",
            path.display()
        ))
    })?;
    apply_steps(ctx, &dc_ops::parse_plan(&text)?)
}

pub(crate) fn apply_steps(ctx: &Ctx<'_>, steps: &[Step]) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(true)?;
    let applied = dc_ops::apply(
        ctx.registry,
        &mut workspace.project,
        &mut workspace.history,
        steps,
        ctx.globals.dry_run,
    )?;
    finish(&mut workspace, &applied)
}

/// Applies `commands` built by the caller (after it prepared files in `workspace`) as one undo
/// step named `name`, like an op.
pub(crate) fn apply_commands(
    ctx: &Ctx<'_>,
    workspace: &mut Workspace,
    name: &str,
    commands: &[&dyn Command],
) -> Result<Answer, CliError> {
    let applied = dc_ops::apply_commands(
        &mut workspace.project,
        &mut workspace.history,
        name,
        commands,
        ctx.globals.dry_run,
    )?;
    finish(workspace, &applied)
}

/// Writes an applied change (history event, then the project) and makes the answer.
fn finish(workspace: &mut Workspace, applied: &Applied) -> Result<Answer, CliError> {
    write(workspace, applied)?;
    let json = applied_json(applied);
    Ok(answer(workspace, applied.to_string(), json))
}

/// Writes an applied change: its history event, then the project. A dry run writes nothing.
pub(crate) fn write(workspace: &Workspace, applied: &Applied) -> Result<(), CliError> {
    if applied.changed && !applied.dry_run {
        let change = workspace.history.changes().last().cloned();
        let change = change.ok_or_else(|| {
            CliError::Failed(
                "внутренняя ошибка: изменение не попало в историю. Это баг, сообщи о нём"
                    .to_owned(),
            )
        })?;
        workspace.record(&[Event::Do(change)])?;
    }
    Ok(())
}

pub(crate) fn applied_json(applied: &Applied) -> Value {
    let (before, after) = applied.duration;
    json!({
        "changed": applied.changed,
        "dry_run": applied.dry_run,
        "diff": applied.diff.to_string(),
        "duration": [before.as_secs_f64(), after.as_secs_f64()],
    })
}

/// Which way `undo` and `redo` move through history.
#[derive(Clone, Copy)]
pub(crate) enum Way {
    Undo,
    Redo,
}

impl Way {
    fn available(self, history: &History) -> usize {
        match self {
            Self::Undo => history.changes().len(),
            Self::Redo => history.redo_changes().len(),
        }
    }

    fn step(self, history: &mut History, project: &mut Project) -> Option<Change> {
        match self {
            Self::Undo => history.undo(project).cloned(),
            Self::Redo => history.redo(project).cloned(),
        }
    }
}

/// `devcast undo [n]`, `devcast redo`.
pub(crate) fn travel(ctx: &Ctx<'_>, way: Way, steps: usize) -> Result<Answer, CliError> {
    let (done, nothing) = match way {
        Way::Undo => ("отменено", "нечего отменять"),
        Way::Redo => ("возвращено", "нечего возвращать"),
    };
    if steps == 0 {
        return Err(CliError::Usage(
            "число шагов — от 1, например devcast undo 2".to_owned(),
        ));
    }
    let mut workspace = ctx.open(true)?;
    let available = way.available(&workspace.history);
    if available < steps {
        return Err(CliError::Failed(match available {
            0 => format!("{nothing}: таких шагов в истории нет"),
            n => format!("{nothing} столько: доступно шагов — {n}; список: devcast history"),
        }));
    }
    let before = TimeMap::build(&workspace.project).duration();
    let mut summaries = Vec::new();
    for _ in 0..steps {
        let change = way.step(&mut workspace.history, &mut workspace.project);
        summaries.extend(change.map(|c| c.summary));
    }
    let after = TimeMap::build(&workspace.project).duration();
    if !ctx.globals.dry_run {
        let event = match way {
            Way::Undo => Event::Undo,
            Way::Redo => Event::Redo,
        };
        workspace.record(&vec![event; steps])?;
    }
    let mut text = format!("{}{done}: {}", ctx.prefix(), summaries.join("; "));
    if let Some(change) = duration_change(before, after) {
        text.push_str(&format!("; {change}"));
    }
    let json = json!({
        done: summaries,
        "dry_run": ctx.globals.dry_run,
        "duration": [before.as_secs_f64(), after.as_secs_f64()],
    });
    Ok(answer(&mut workspace, text, json))
}

/// `devcast history`: steps `undo` would revert and `redo` would bring back, newest first.
pub(crate) fn history(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(false)?;
    let undo: Vec<&Change> = workspace.history.changes().iter().rev().collect();
    let redo: Vec<&Change> = workspace.history.redo_changes().iter().rev().collect();
    let width = undo
        .iter()
        .chain(&redo)
        .map(|c| c.name.chars().count())
        .max()
        .unwrap_or(0);
    let digits = undo.len().max(redo.len()).to_string().len();
    let mut lines = Vec::new();
    for (title, changes) in [
        ("отменить (devcast undo N):", &undo),
        ("вернуть (devcast redo):", &redo),
    ] {
        if !changes.is_empty() {
            lines.push(title.to_owned());
        }
        for (index, change) in changes.iter().enumerate() {
            let name = &change.name;
            lines.push(format!(
                "  {:>digits$}  {name:width$}  {}",
                index + 1,
                change.summary
            ));
        }
    }
    if lines.is_empty() {
        lines.push("история пуста".to_owned());
    }
    let list = |changes: &[&Change]| -> Vec<Value> {
        changes
            .iter()
            .map(|c| json!({"name": c.name, "summary": c.summary}))
            .collect()
    };
    let json = json!({"undo": list(&undo), "redo": list(&redo)});
    Ok(answer(&mut workspace, lines.join("\n"), json))
}

/// `devcast save`: unsaved editor state (`session.json`) becomes the saved project.
pub(crate) fn save(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(true)?;
    let unsaved = workspace.unsaved();
    let text = match (unsaved, ctx.globals.dry_run) {
        (false, _) => "нечего сохранять: несохранённых изменений нет".to_owned(),
        (true, true) => format!("{DRY_RUN}рабочее состояние сохранится в project.json"),
        (true, false) => {
            workspace.save()?;
            "сохранено в project.json".to_owned()
        }
    };
    let json = json!({"saved": unsaved && !ctx.globals.dry_run, "dry_run": ctx.globals.dry_run});
    Ok(answer(&mut workspace, text, json))
}
