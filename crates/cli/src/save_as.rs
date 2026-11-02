//! `devcast save --as <name>`: a copy of the project's working state with its recordings and
//! assets as `<name>.devcast` next to it (`ProjectDir::save_as`, the copy of «Сохранить как…»).
//!
//! The run makes the copy itself, also of a project the editor holds: `Open` has put the
//! editor's exact working state on disk, so the copy is made of the same files by the same code
//! as without the editor, and the editor window does not stop while it is made (copying the
//! recordings can take long, and the app answers requests in its frame). The editor then does
//! what it does after «Сохранить как…» (`Request::Copied`): drops its working state and opens the
//! copy; if it changed after the run read it, it refuses and the copy is deleted. Without the
//! editor the original keeps its working state and `--open` opens the copy in the app.

use std::path::Path;

use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::open::open_in_app;
use crate::run::Answer;

pub(crate) fn save_as(ctx: &Ctx<'_>, name: &str, open: bool) -> Result<Answer, CliError> {
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let target = dir.renamed_root(name)?;
    let to = folder_name(&target);
    if ctx.globals.dry_run {
        let text = format!("{}копия «{}» → «{to}» рядом", ctx.prefix(), dir.name());
        return Ok(Answer::new(
            text,
            answer_json(&dir, &target, false, false, true),
        ));
    }
    let workspace = ctx.open(false)?;
    dir.save_as(&workspace.project, &target)?;
    let (opened, asking) = match workspace.editor() {
        Some(editor) => {
            if let Err(error) = editor.copied(target.clone()) {
                return Err(remove_copy(&target, error));
            }
            (true, false)
        }
        None if open => {
            let asking = open_in_app(ctx, &target)?;
            (!asking, asking)
        }
        None => (false, false),
    };
    let mut text = format!("копия «{}» → «{to}»", dir.name());
    if opened {
        text.push_str(": открыта в редакторе");
    } else if asking {
        text.push_str(
            ": редактор спрашивает, сохранить ли правки открытого проекта, потом откроет копию",
        );
    }
    let mut answer = Answer::new(text, answer_json(&dir, &target, opened, asking, false));
    answer.warnings = workspace.warnings;
    Ok(answer)
}

fn answer_json(
    dir: &dc_project::ProjectDir,
    target: &Path,
    opened: bool,
    asking: bool,
    dry_run: bool,
) -> serde_json::Value {
    json!({
        "from": dir.name(),
        "to": folder_name(target),
        "path": target,
        "opened": opened,
        "asking": asking,
        "dry_run": dry_run,
    })
}

fn folder_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The editor refused the copy: it goes away, the error stays.
fn remove_copy(target: &Path, error: CliError) -> CliError {
    match std::fs::remove_dir_all(target) {
        Ok(()) => error,
        Err(e) => CliError::Failed(format!(
            "{error}; копию {} удалить не удалось ({e}): удали её вручную",
            target.display()
        )),
    }
}
