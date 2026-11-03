//! `devcast app feedback`: «О программе» of the settings window — where to write about a bug or
//! an idea (`crates/update/feedback.json`) and the folder of the logs to send along.

use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;

pub(crate) fn feedback(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let feedback = dc_update::feedback().map_err(|e| CliError::Failed(e.to_string()))?;
    let logs = ctx.settings_dir()?.join(dc_project::settings::LOGS_DIR);
    let mut lines = vec!["Нашли баг или есть идея — напишите:".to_owned()];
    lines.extend(
        feedback
            .contacts
            .iter()
            .map(|c| format!("{}: {}", c.name, c.value)),
    );
    lines.push(format!("Репозиторий: {}", feedback.repository));
    lines.push(format!("Логи (приложите к письму): {}", logs.display()));
    let contacts: Vec<_> = feedback
        .contacts
        .iter()
        .map(|c| json!({"name": c.name, "value": c.value, "url": c.url}))
        .collect();
    let json = json!({
        "contacts": contacts,
        "repository": feedback.repository,
        "logs": logs,
    });
    Ok(Answer::new(lines.join("\n"), json))
}
