//! `devcast ls projects`: the projects of the editor's «Проекты» menu (the last ones opened,
//! `recent_projects` of the app settings, folders still there) and of the recordings folder (the
//! editor's library, `dc_project::library`). The running app tells which one its editor holds.

use std::path::{Path, PathBuf};

use dc_ops::live::{Answer as Live, Request};
use serde_json::{Value, json};

use crate::edit::Ctx;
use crate::error::CliError;
use crate::live::{app_channel, ask_app, refused};
use crate::run::Answer;

/// The project the editor of the running app holds; none without the app.
fn open_in_editor(ctx: &Ctx<'_>) -> Result<Option<PathBuf>, CliError> {
    let channel = app_channel(ctx.local.app_channel)?;
    match ask_app(&channel, &Request::Current)? {
        Some(Live::Current { root }) => Ok(root),
        Some(answer) => Err(refused(answer)),
        None => Ok(None),
    }
}

fn same(a: &Path, b: Option<&PathBuf>) -> bool {
    b.is_some_and(
        |b| match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        },
    )
}

fn name(root: &Path) -> String {
    root.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) fn ls(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let settings = ctx.app_settings()?;
    let open = open_in_editor(ctx)?;
    let mark = |root: &Path| {
        if same(root, open.as_ref()) {
            " (открыт в редакторе)"
        } else {
            ""
        }
    };
    let recent: Vec<&PathBuf> = settings
        .recent_projects
        .iter()
        .filter(|root| root.is_dir())
        .collect();
    let mut lines = vec!["последние:".to_owned()];
    if recent.is_empty() {
        lines.push("  нет".to_owned());
    }
    for root in &recent {
        lines.push(format!(
            "  {}  {}{}",
            name(root),
            root.display(),
            mark(root)
        ));
    }
    let folder = &settings.recordings_dir;
    let scan = dc_project::library::scan(folder);
    lines.push(format!("папка записей {}:", folder.display()));
    if scan.entries.is_empty() {
        lines.push("  нет".to_owned());
    }
    for entry in &scan.entries {
        lines.push(format!(
            "  {}  {}  {}{}",
            entry.name,
            dc_core::OutputTime::ZERO + entry.duration,
            entry.root.display(),
            mark(&entry.root)
        ));
    }
    let recent_json: Vec<Value> = recent
        .iter()
        .map(|root| json!({"name": name(root), "path": root, "open": same(root, open.as_ref())}))
        .collect();
    let recordings: Vec<Value> = scan
        .entries
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "path": e.root,
                "duration": e.duration.as_secs_f64(),
                "imported": e.imported,
                "open": same(&e.root, open.as_ref()),
            })
        })
        .collect();
    let mut answer = Answer::new(
        lines.join("\n"),
        json!({"recent": recent_json, "recordings_dir": folder, "recordings": recordings, "open": open}),
    );
    answer.warnings = scan
        .skipped
        .iter()
        .map(|(root, error)| format!("{} пропущен: {error}", root.display()))
        .collect();
    Ok(answer)
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
