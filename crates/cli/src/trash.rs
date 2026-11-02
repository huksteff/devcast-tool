//! `devcast trash <project…>` moves project folders to the Windows Recycle Bin, never deletes
//! them; `devcast trash --restore <project…>` brings them back to where they were. The same
//! `dc_project::bin` as the bin of the editor's library. A project another writer holds (the
//! editor, a recording) is refused: its lock is taken and let go right before the move, because
//! Windows moves no folder with a file open in it.

use std::path::{Path, PathBuf};

use dc_project::{ProjectDir, ProjectError, ProjectLock};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::open::absolute;
use crate::run::Answer;

pub(crate) fn trash(
    ctx: &Ctx<'_>,
    projects: &[PathBuf],
    restore: bool,
) -> Result<Answer, CliError> {
    let roots: Vec<PathBuf> = projects
        .iter()
        .map(|p| absolute(&ctx.cwd.join(p)))
        .collect();
    let dry_run = ctx.globals.dry_run;
    match (restore, dry_run) {
        (false, _) => {
            let dirs = roots
                .iter()
                .map(ProjectDir::open)
                .collect::<Result<Vec<_>, _>>()?;
            let locks = dirs
                .iter()
                .map(|dir| ProjectLock::acquire(dir, "devcast trash").map_err(|e| held(dir, e)))
                .collect::<Result<Vec<_>, _>>()?;
            drop(locks);
            if !dry_run {
                dc_project::bin::trash(&roots)?;
            }
        }
        (true, true) => dc_project::bin::check_restore(&roots)?,
        (true, false) => dc_project::bin::restore(&roots)?,
    }
    let names = roots
        .iter()
        .map(|r| format!("«{}»", name(r)))
        .collect::<Vec<_>>()
        .join(", ");
    let text = match (restore, dry_run) {
        (false, true) => format!("{}в корзину Windows переместятся: {names}", ctx.prefix()),
        (false, false) => format!(
            "в корзине Windows: {names}; вернуть — devcast trash --restore с теми же путями"
        ),
        (true, true) => format!("{}из корзины Windows вернутся: {names}", ctx.prefix()),
        (true, false) => format!("из корзины Windows вернулись: {names}"),
    };
    let key = if restore { "restored" } else { "trashed" };
    let json = json!({ key: roots, "dry_run": dry_run });
    Ok(Answer::new(text, json))
}

fn name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A project another writer holds stays where it is.
fn held(dir: &ProjectDir, error: ProjectError) -> CliError {
    match error {
        ProjectError::Locked { holder } => CliError::Failed(format!(
            "«{}» открыт: {holder}. Закрой его (devcast close --project <путь>) и повтори",
            dir.name()
        )),
        error => error.into(),
    }
}
