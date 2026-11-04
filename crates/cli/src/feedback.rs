//! `devcast app feedback`: «О программе» of the settings window — where to write about a bug or
//! an idea (`crates/update/feedback.json`), the folder of the logs to send along and the crash
//! reports in it (`logs\crashes`, what «DevCast в прошлый раз упал» of the app tells about).

use std::path::{Path, PathBuf};

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
    if !feedback.authors.is_empty() {
        lines.push("Авторы:".to_owned());
        lines.extend(
            feedback
                .authors
                .iter()
                .map(|a| format!("  {} {}", a.name, a.tag)),
        );
    }
    lines.push(format!("Репозиторий: {}", feedback.repository));
    lines.push(format!("Логи (приложите к письму): {}", logs.display()));
    let crashes = reports(&logs.join(dc_project::settings::CRASHES_DIR))?;
    if !crashes.is_empty() {
        lines.push(format!("Отчётов о падениях в логах: {}", crashes.len()));
    }
    let contacts: Vec<_> = feedback
        .contacts
        .iter()
        .map(|c| json!({"name": c.name, "value": c.value, "url": c.url}))
        .collect();
    let json = json!({
        "contacts": contacts,
        "authors": feedback
            .authors
            .iter()
            .map(|a| json!({"name": a.name, "tag": a.tag}))
            .collect::<Vec<_>>(),
        "repository": feedback.repository,
        "logs": logs,
        "crashes": crashes,
    });
    Ok(Answer::new(lines.join("\n"), json))
}

/// The crash reports in `dir`, oldest first; none without the folder.
fn reports(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    let failed =
        |e: std::io::Error| CliError::Failed(format!("не читается {}: {e}", dir.display()));
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(failed(error)),
    };
    let mut files = Vec::new();
    for entry in entries {
        files.push(entry.map_err(failed)?.path());
    }
    files.sort();
    Ok(files)
}
