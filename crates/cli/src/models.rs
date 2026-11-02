//! `devcast models ls|download`: the Whisper models of «Быстрая» (`fast`) and «Точная»
//! (`accurate`) in the models folder of the app settings — the same files the settings window
//! and the editor's «Создать субтитры» download and `captions gen` uses. The download is
//! `dc_models::ensure`, as `speech.rs` downloads: resumable, SHA-256 checked.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use dc_models::Progress;
use dc_settings::SpeechModel;
use serde_json::{Value, json};

use crate::config::model_name;
use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;

/// Progress lines go out each this many percent.
const STEP: u64 = 10;

fn path(ctx: &Ctx<'_>, model: SpeechModel) -> Result<PathBuf, CliError> {
    Ok(ctx.models_dir()?.join(model.model().file_name()))
}

/// Bytes on disk of a downloaded model.
fn size(path: &std::path::Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|m| m.len())
}

fn megabytes(bytes: u64) -> u64 {
    bytes / 1_000_000
}

/// `models ls`: each model, whether it is on disk, its size and file.
pub(crate) fn ls(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let mut lines = Vec::new();
    let mut items = Vec::new();
    for model in SpeechModel::ALL {
        let path = path(ctx, model)?;
        let on_disk = size(&path);
        let state = match on_disk {
            Some(bytes) => format!("скачана, {} МБ", megabytes(bytes)),
            None => format!("не скачана, ≈ {} МБ", model.model().megabytes()),
        };
        lines.push(format!(
            "{:<9} {state}  {}",
            model_name(model),
            path.display()
        ));
        items.push(json!({
            "name": model_name(model),
            "file": model.model().file_name(),
            "ready": on_disk.is_some(),
            "size": on_disk,
            "megabytes": model.model().megabytes(),
            "path": path,
        }));
    }
    Ok(Answer::new(lines.join("\n"), Value::Array(items)))
}

/// The progress line of `done`, when it passed the next [`STEP`] after `shown` (percent).
fn progress_line(name: &str, done: Progress, shown: &mut Option<u64>) -> Option<String> {
    let total = done.total.filter(|t| *t > 0)?;
    let percent = (done.done.min(total) * 100 / total) / STEP * STEP;
    if shown.is_some_and(|s| s >= percent) {
        return None;
    }
    *shown = Some(percent);
    Some(format!(
        "{name}: {percent}% ({} из {} МБ)",
        megabytes(done.done),
        megabytes(total)
    ))
}

/// `models download fast|accurate`: progress lines while it downloads (`Local::progress`).
pub(crate) fn download(ctx: &Ctx<'_>, model: SpeechModel) -> Result<Answer, CliError> {
    let name = model_name(model);
    let path = path(ctx, model)?;
    let json = |downloaded: bool| json!({"name": name, "path": path, "downloaded": downloaded, "dry_run": ctx.globals.dry_run});
    if size(&path).is_some() {
        let text = format!("{name} уже скачана: {}", path.display());
        return Ok(Answer::new(text, json(false)));
    }
    if ctx.globals.dry_run {
        let text = format!(
            "{}скачается {name} (≈ {} МБ) в {}",
            ctx.prefix(),
            model.model().megabytes(),
            path.display()
        );
        return Ok(Answer::new(text, json(false)));
    }
    let mut shown = None;
    let mut on_progress = |done: Progress| {
        if let (Some(out), Some(line)) = (ctx.local.progress, progress_line(name, done, &mut shown))
        {
            out(&line);
        }
    };
    let cancel = AtomicBool::new(false);
    dc_models::ensure(model.model(), &ctx.models_dir()?, &mut on_progress, &cancel)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let text = format!("{name} скачана: {}", path.display());
    Ok(Answer::new(text, json(true)))
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
