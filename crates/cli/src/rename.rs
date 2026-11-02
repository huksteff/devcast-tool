//! `devcast rename <name>`: the project folder becomes `<name>.devcast` next to it
//! (`ProjectDir::rename`, the path of renaming in the editor's header). No undo: a folder name is
//! not a change of `project.json`. A project the editor holds is renamed by the editor
//! (`Request::Rename`): it lets go of its files, renames and opens the project again, and refuses
//! while its export queue reads the project.

use std::path::PathBuf;

use dc_ops::live::{Answer as Live, Request};
use dc_project::{ProjectDir, ProjectLock};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::live::{ask_app, held_by_app, refused};
use crate::open::absolute;
use crate::run::Answer;

pub(crate) fn rename(ctx: &Ctx<'_>, words: &[String]) -> Result<Answer, CliError> {
    let name = words.join(" ");
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let mut target = dir.renamed_root(&name)?;
    if !ctx.globals.dry_run {
        target = match rename_in_app(ctx, &dir, &name)? {
            Some(root) => root,
            None => {
                let lock = ProjectLock::acquire(&dir, "devcast")?;
                dir.rename(lock, &name)?.root().to_path_buf()
            }
        };
    }
    let to = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = format!("{}«{}» → «{to}»", ctx.prefix(), dir.name());
    let json = json!({
        "from": dir.name(),
        "to": to,
        "path": target,
        "dry_run": ctx.globals.dry_run,
    });
    Ok(Answer::new(text, json))
}

/// Asks the editor that holds `dir`; `None` when none does.
fn rename_in_app(ctx: &Ctx<'_>, dir: &ProjectDir, name: &str) -> Result<Option<PathBuf>, CliError> {
    let Some(channel) = held_by_app(dir, ctx.local.app_channel)? else {
        return Ok(None);
    };
    let request = Request::Rename {
        root: absolute(dir.root()),
        name: name.to_owned(),
    };
    match ask_app(&channel, &request)? {
        Some(Live::Renamed { root }) => Ok(Some(root)),
        Some(Live::NotOpen) | None => Ok(None),
        Some(answer) => Err(refused(answer)),
    }
}
