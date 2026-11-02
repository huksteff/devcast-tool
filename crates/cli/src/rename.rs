//! `devcast rename <name>`: the project folder becomes `<name>.devcast` next to it
//! (`ProjectDir::rename`, the path of renaming in the editor's header). No undo: a folder name is
//! not a change of `project.json`. Refused while another writer (the editor) holds the project.

use dc_project::ProjectLock;
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::run::Answer;

pub(crate) fn rename(ctx: &Ctx<'_>, words: &[String]) -> Result<Answer, CliError> {
    let name = words.join(" ");
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let target = dir.renamed_root(&name)?;
    if !ctx.globals.dry_run {
        let lock = ProjectLock::acquire(&dir, "devcast")?;
        dir.rename(lock, &name)?;
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
