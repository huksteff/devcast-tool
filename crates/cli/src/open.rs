//! `devcast open`: the running app opens the project in its editor (an editor with unsaved
//! changes of another project asks about them in its window first). With it an agent reaches the
//! editor without the HUD; what the editor then does is a `devcast` command as well.

use std::path::{Path, PathBuf};

use dc_ops::live::{Answer as Live, Request};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::live::{NOT_RUNNING, app_channel, ask_app, refused};
use crate::run::Answer;

pub(crate) fn open(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let root = absolute(dir.root());
    let name = dir.name().to_owned();
    if ctx.globals.dry_run {
        let text = format!("{}«{name}» откроется в редакторе", ctx.prefix());
        let json = json!({ "path": root, "opened": false, "asking": false, "dry_run": true });
        return Ok(Answer::new(text, json));
    }
    let asking = open_in_app(ctx, &root)?;
    let text = match asking {
        false => format!("«{name}» открыт в редакторе"),
        true => format!(
            "редактор спрашивает, сохранить ли правки открытого проекта: после ответа в окне откроется «{name}»"
        ),
    };
    let json = json!({ "path": root, "opened": !asking, "asking": asking, "dry_run": false });
    Ok(Answer::new(text, json))
}

/// Asks the running app to open the project in `root` in its editor; true when the editor asks
/// about the unsaved changes of its project first.
pub(crate) fn open_in_app(ctx: &Ctx<'_>, root: &Path) -> Result<bool, CliError> {
    let channel = app_channel(ctx.local.app_channel)?;
    let request = Request::Edit {
        root: root.to_path_buf(),
    };
    match ask_app(&channel, &request)? {
        Some(Live::Opened) => Ok(false),
        Some(Live::Asking) => Ok(true),
        Some(answer) => Err(refused(answer)),
        None => Err(CliError::Failed(NOT_RUNNING.to_owned())),
    }
}

/// `root` as an absolute path: the app compares folders of other runs.
pub(crate) fn absolute(root: &Path) -> PathBuf {
    std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf())
}
