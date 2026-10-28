//! One run of `devcast`: arguments in; text for stdout and stderr and an exit code out.
//! stdout gets only the result, stderr errors and warnings. Exit codes: 0 done, 1 failed,
//! 2 wrong command line.

use std::path::Path;

use clap::{Parser, Subcommand};
use dc_ops::{Applied, Registry, Step};
use dc_project::history_log::Event;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::find::find_project;
use crate::globals::{self, Globals};
use crate::help::help;
use crate::workspace::Workspace;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    pub fn failure(error: &CliError, json: bool) -> Self {
        let text = match json {
            true => json!({"error": error.to_string(), "code": error.code()}).to_string(),
            false => error.to_string(),
        };
        Self {
            code: error.code(),
            stdout: String::new(),
            stderr: text + "\n",
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "devcast",
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Builtin,
}

/// Built-in commands. `about` is the line in `devcast help`, `after_help` the example.
#[derive(Debug, Subcommand)]
enum Builtin {
    #[command(
        about = "Список команд; с именем команды — её аргументы и пример",
        after_help = "help zoom add"
    )]
    Help {
        #[arg(help = "Команда из нескольких слов, например zoom add")]
        command: Vec<String>,
    },
    /// An op from the registry, e.g. `zoom add --at 12.5`.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

/// What a successful command prints: `text` normally, `json` under `--json`.
pub(crate) struct Answer {
    text: String,
    json: Value,
    warnings: Vec<String>,
}

impl Answer {
    pub(crate) fn new(text: String, json: Value) -> Self {
        Self {
            text,
            json,
            warnings: Vec::new(),
        }
    }
}

pub fn run(args: Vec<String>, cwd: &Path, registry: &Registry) -> Outcome {
    let (globals, rest) = globals::split(args);
    match rest.and_then(|rest| dispatch(rest, &globals, cwd, registry)) {
        Ok(answer) => Outcome {
            code: 0,
            stdout: match globals.json {
                true => answer.json.to_string(),
                false => answer.text,
            } + "\n",
            stderr: answer
                .warnings
                .iter()
                .map(|w| format!("внимание: {w}\n"))
                .collect(),
        },
        Err(error) => Outcome::failure(&error, globals.json),
    }
}

fn dispatch(
    rest: Vec<String>,
    globals: &Globals,
    cwd: &Path,
    registry: &Registry,
) -> Result<Answer, CliError> {
    if rest.is_empty() {
        return Err(CliError::Usage(
            "нужна команда, например: devcast zoom add --at 12.5. Список: devcast help".to_owned(),
        ));
    }
    let cli =
        Cli::try_parse_from(std::iter::once("devcast".to_owned()).chain(rest)).map_err(|e| {
            CliError::Usage(format!(
                "командная строка не разобрана ({}). Список: devcast help",
                e.kind()
            ))
        })?;
    match cli.command {
        Builtin::Help { command } => help(&command, registry),
        Builtin::Op(words) => op(&words, globals, cwd, registry),
    }
}

/// `devcast <op> [args]`: one op applied to the project as one undo step.
fn op(
    words: &[String],
    globals: &Globals,
    cwd: &Path,
    registry: &Registry,
) -> Result<Answer, CliError> {
    let (info, taken) = registry.resolve(words)?;
    let fields = dc_ops::parse_args(info, &words[taken..])?;
    let steps = [Step::new(info.name, fields)];
    let dir = find_project(globals.project.as_deref(), cwd)?;
    let mut workspace = Workspace::open(dir, !globals.dry_run)?;
    let applied = dc_ops::apply(
        registry,
        &mut workspace.project,
        &mut workspace.history,
        &steps,
        globals.dry_run,
    )?;
    if applied.changed && !applied.dry_run {
        let change = workspace.history.changes().last().cloned().ok_or_else(|| {
            CliError::Failed(
                "внутренняя ошибка: изменение не попало в историю. Это баг, сообщи о нём"
                    .to_owned(),
            )
        })?;
        workspace.record(Event::Do(change))?;
    }
    Ok(Answer {
        text: applied.to_string(),
        json: applied_json(&applied),
        warnings: std::mem::take(&mut workspace.warnings),
    })
}

fn applied_json(applied: &Applied) -> Value {
    let (before, after) = applied.duration;
    json!({
        "changed": applied.changed,
        "dry_run": applied.dry_run,
        "diff": applied.diff.to_string(),
        "duration": [before.as_secs_f64(), after.as_secs_f64()],
    })
}
