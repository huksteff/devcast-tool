//! `devcast discard` and `devcast close`: the unsaved working state and the editor window. A
//! project the editor holds is asked of it (`dc_ops::live`), the way its own buttons do it
//! («Отбросить», closing the window); without the editor `discard` works on the files under the
//! lock (`Working::discard`, the same function as `Document::discard`).

use dc_ops::live::{Answer as Live, Changes, Request};
use dc_project::{ProjectDir, ProjectLock, Working};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::live::{NOT_RUNNING, app_channel, ask_app, held_by_app, refused};
use crate::open::absolute;
use crate::run::Answer;

const NOTHING: &str = "отбрасывать нечего: несохранённых правок нет";

/// `devcast discard`: back to `project.json`, the history of undo cleared.
pub(crate) fn discard(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let changed = match ctx.globals.dry_run {
        true => ctx.open(false)?.unsaved(),
        false => match discard_in_app(ctx, &dir)? {
            Some(changed) => changed,
            None => discard_files(&dir)?,
        },
    };
    let text = match (changed, ctx.globals.dry_run) {
        (false, _) => NOTHING.to_owned(),
        (true, true) => format!(
            "{}несохранённые правки «{}» отбросятся: проект вернётся к project.json, история отмен очистится",
            ctx.prefix(),
            dir.name()
        ),
        (true, false) => format!(
            "несохранённые правки «{}» отброшены: проект как в project.json, история отмен очищена",
            dir.name()
        ),
    };
    let json = json!({
        "discarded": changed && !ctx.globals.dry_run,
        "unsaved": changed,
        "dry_run": ctx.globals.dry_run,
    });
    Ok(Answer::new(text, json))
}

/// Asks the editor that holds `dir`; `None` when none does (the app is gone or has another).
fn discard_in_app(ctx: &Ctx<'_>, dir: &ProjectDir) -> Result<Option<bool>, CliError> {
    let Some(channel) = held_by_app(dir, ctx.local.app_channel)? else {
        return Ok(None);
    };
    let request = Request::Discard {
        root: absolute(dir.root()),
    };
    match ask_app(&channel, &request)? {
        Some(Live::Discarded { changed }) => Ok(Some(changed)),
        Some(Live::NotOpen) | None => Ok(None),
        Some(answer) => Err(refused(answer)),
    }
}

fn discard_files(dir: &ProjectDir) -> Result<bool, CliError> {
    let lock = ProjectLock::acquire(dir, "devcast")?;
    if !dir.has_unsaved_changes() {
        return Ok(false);
    }
    Working::discard(dir, &lock)?;
    Ok(true)
}

/// `devcast close [--save | --discard]`: the editor window of the project closes, the HUD comes
/// back.
pub(crate) fn close(ctx: &Ctx<'_>, changes: Option<Changes>) -> Result<Answer, CliError> {
    let dir = match find_project(ctx.globals.project.as_deref(), ctx.cwd) {
        Ok(dir) => dir,
        // Without --project outside a project: the window of whatever the editor has open.
        Err(error) if ctx.globals.project.is_none() => {
            match crate::projects::open_in_editor(ctx)? {
                Some(root) => ProjectDir::open(root)?,
                None => return Err(error),
            }
        }
        Err(error) => return Err(error),
    };
    let name = dir.name().to_owned();
    let what = match changes {
        None => None,
        Some(Changes::Save) => Some("save"),
        Some(Changes::Discard) => Some("discard"),
    };
    let json =
        json!({ "closed": !ctx.globals.dry_run, "changes": what, "dry_run": ctx.globals.dry_run });
    if ctx.globals.dry_run {
        let text = format!("{}окно редактора с «{name}» закроется", ctx.prefix());
        return Ok(Answer::new(text, json));
    }
    let channel = app_channel(ctx.local.app_channel)?;
    let request = Request::Close {
        root: absolute(dir.root()),
        changes,
    };
    match ask_app(&channel, &request)? {
        Some(Live::Closed) => {}
        None => return Err(CliError::Failed(NOT_RUNNING.to_owned())),
        Some(Live::Unsaved) => {
            return Err(CliError::Failed(format!(
                "в «{name}» есть несохранённые правки: добавь --save или --discard"
            )));
        }
        Some(Live::Elsewhere { open: Some(open) }) => {
            let open = open
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            return Err(CliError::Failed(format!(
                "в редакторе открыт «{open}», а не «{name}»: закрой его, указав --project <путь к нему>"
            )));
        }
        Some(Live::Elsewhere { open: None }) => {
            return Err(CliError::Failed(
                "в DevCast не открыт ни один проект: закрывать нечего".to_owned(),
            ));
        }
        Some(answer) => return Err(refused(answer)),
    }
    let text = format!("окно редактора закрыто: «{name}»");
    Ok(Answer::new(text, json))
}
