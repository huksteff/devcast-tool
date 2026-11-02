//! `devcast open`: the running app opens the project in its editor (an editor with unsaved
//! changes of another project asks about them in its window first). With it an agent reaches the
//! editor without the HUD; what the editor then does is a `devcast` command as well.

use dc_ipc::IpcError;
use dc_ops::live::{Answer as Live, Request, SERVICE};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::run::Answer;

pub(crate) fn open(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let root = std::path::absolute(dir.root()).unwrap_or_else(|_| dir.root().to_path_buf());
    let name = dir.name().to_owned();
    if ctx.globals.dry_run {
        let text = format!("{}«{name}» откроется в редакторе", ctx.prefix());
        let json = json!({ "path": root, "opened": false, "asking": false, "dry_run": true });
        return Ok(Answer::new(text, json));
    }
    let channel = match ctx.local.app_channel {
        Some(channel) => channel.to_owned(),
        None => dc_ipc::socket_name(SERVICE).map_err(lost)?,
    };
    let answer = dc_ipc::request(&channel, &Request::Edit { root: root.clone() });
    let asking = match answer {
        Ok(Live::Opened) => false,
        Ok(Live::Asking) => true,
        Ok(Live::Refused(text)) => return Err(CliError::Failed(text)),
        Ok(other) => {
            return Err(CliError::Failed(format!(
                "программа ответила не по делу ({other:?}): обнови DevCast, чтобы программа и devcast были одной версии"
            )));
        }
        Err(IpcError::NotRunning(_)) => {
            return Err(CliError::Failed(
                "DevCast не запущен: запусти программу и повтори команду".to_owned(),
            ));
        }
        Err(error) => return Err(lost(error)),
    };
    let text = match asking {
        false => format!("«{name}» открыт в редакторе"),
        true => format!(
            "редактор спрашивает, сохранить ли правки открытого проекта: после ответа в окне откроется «{name}»"
        ),
    };
    let json = json!({ "path": root, "opened": !asking, "asking": asking, "dry_run": false });
    Ok(Answer::new(text, json))
}

fn lost(error: IpcError) -> CliError {
    CliError::Failed(format!(
        "связаться с программой DevCast не удалось ({error}): перезапусти её и повтори команду"
    ))
}
