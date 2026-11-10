//! What a `devcast` run by the user's AI agent («Сделать красиво», `DEVCAST_GROUP`) may touch:
//! only the project of the folder it was started in (the one `devcast` finds without `--project`). Commands about the project as a whole and
//! the program are closed (`Cli::not_for_agent`), `--project` may not name another project, and
//! files are written only into the project folder or the temp folder: the text of a project
//! (speech, annotations) could otherwise talk the agent into overwriting the user's files.

use std::path::{Path, PathBuf};

use crate::builtin::Cli;
use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;

/// Refuses what the agent may not do.
pub(crate) fn check(ctx: &Ctx<'_>, cli: &Cli) -> Result<(), CliError> {
    if let Some(why) = cli.not_for_agent() {
        return Err(closed(&format!("эта команда закрыта: {why}")));
    }
    let project = canonical(find_project(None, ctx.cwd)?.root())?;
    if let Some(other) = &ctx.globals.project
        && canonical(&ctx.cwd.join(other)).ok().as_ref() != Some(&project)
    {
        return Err(closed(
            "правится только проект текущей папки: убери --project",
        ));
    }
    if let Some(out) = cli.written() {
        let file = ctx.cwd.join(out);
        let folder = canonical(file.parent().unwrap_or(ctx.cwd))?;
        let temp = canonical(&std::env::temp_dir())?;
        if !folder.starts_with(&project) && !folder.starts_with(&temp) {
            return Err(closed(&format!(
                "файлы пишутся только в папку проекта или во временную папку {}: поменяй -o",
                temp.display()
            )));
        }
    }
    Ok(())
}

fn closed(why: &str) -> CliError {
    CliError::Usage(format!("во время «Сделать красиво» {why}"))
}

fn canonical(path: &Path) -> Result<PathBuf, CliError> {
    std::fs::canonicalize(path).map_err(|error| {
        CliError::Failed(format!(
            "папка {} не открылась ({error}): проверь путь",
            path.display()
        ))
    })
}
