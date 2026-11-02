//! `devcast config show|set|reset`: the app settings of the settings window and the HUD (where
//! recordings go, what to record, the countdown, updates, the captions of the editor panel) as
//! `key=value`. The values are read here; the edit is applied by the app or in the file
//! (`app_settings.rs`).

use std::path::Path;
use std::str::FromStr;

use dc_capture::{Fps, Source};
use dc_settings::{AppSettings, Countdown, Edit, Key, Language, Listen, Setting, SpeechModel};
use serde_json::{Map, Value, json};

use crate::app_settings::{self, argument, switch};
use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;

/// The text of `value`, as `config set` takes it (unquoted).
pub(crate) fn text(value: &Setting) -> String {
    match value {
        Setting::RecordingsDir(dir) => dir.display().to_string(),
        Setting::Source(source) => source.to_string(),
        Setting::Fps(fps) => fps.get().to_string(),
        Setting::Mic(None) => "off".to_owned(),
        Setting::Mic(Some(name)) => name.clone(),
        Setting::Audio(on)
        | Setting::HideHud(on)
        | Setting::AutoUpdates(on)
        | Setting::Beta(on) => switch(*on).to_owned(),
        Setting::Countdown(countdown) => countdown.seconds().to_string(),
        Setting::Model(model) => model_name(*model).to_owned(),
        Setting::Language(language) => language_name(*language).to_owned(),
        Setting::Tracks(tracks) => tracks_name(*tracks).to_owned(),
    }
}

pub(crate) fn model_name(model: SpeechModel) -> &'static str {
    match model {
        SpeechModel::Fast => "fast",
        SpeechModel::Accurate => "accurate",
    }
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::Auto => "auto",
        Language::Russian => "ru",
        Language::English => "en",
    }
}

fn tracks_name(tracks: Listen) -> &'static str {
    match tracks {
        Listen::Mic => "mic",
        Listen::System => "system",
        Listen::Both => "both",
    }
}

/// `fast` or `accurate`: `config set captions.model=`, `--model` of `captions gen` and
/// `transcript`, `models download`.
pub(crate) fn parse_model(text: &str) -> Result<SpeechModel, String> {
    SpeechModel::ALL
        .into_iter()
        .find(|m| model_name(*m) == text)
        .ok_or_else(|| format!("модели «{text}» нет: fast или accurate"))
}

/// What `config set` takes for each key.
fn accepted(key: Key) -> &'static str {
    match key {
        Key::RecordingsDir => "папка",
        Key::Source => "screen:<n>, window:<id> или area:x,y,w,h (devcast ls sources)",
        Key::Fps => "30, 60 или 120",
        Key::Mic => "имя микрофона из devcast ls sources или off",
        Key::Audio | Key::HideHud | Key::AutoUpdates | Key::Beta => "on или off",
        Key::Countdown => "0, 3, 5 или 10",
        Key::Model => "fast или accurate",
        Key::Language => "auto, ru или en",
        Key::Tracks => "mic, system или both",
    }
}

fn unknown(name: &str) -> CliError {
    let keys: Vec<&str> = Key::ALL.iter().map(|k| k.name()).collect();
    CliError::Usage(format!("ключа «{name}» нет; есть: {}", keys.join(", ")))
}

fn on_off(text: &str) -> Option<bool> {
    match text {
        "on" | "true" => Some(true),
        "off" | "false" => Some(false),
        _ => None,
    }
}

/// `key=value` of `config set`; a relative folder is taken from `cwd`.
fn parse(assignment: &str, cwd: &Path) -> Result<Setting, CliError> {
    let Some((name, value)) = assignment.split_once('=') else {
        return Err(CliError::Usage(format!(
            "«{assignment}»: нужно ключ=значение, например recording.fps=60; ключи — devcast config show"
        )));
    };
    let key = Key::find(name).ok_or_else(|| unknown(name))?;
    let bad = || CliError::Usage(format!("{name}={value}: можно {}", accepted(key)));
    let setting = match key {
        Key::RecordingsDir if !value.is_empty() => {
            Setting::RecordingsDir(std::path::absolute(cwd.join(value)).map_err(|_| bad())?)
        }
        Key::RecordingsDir => return Err(bad()),
        Key::Source => Setting::Source(Source::from_str(value).map_err(|_| bad())?),
        Key::Fps => Setting::Fps(
            value
                .parse::<u32>()
                .ok()
                .and_then(|n| Fps::try_from(n).ok())
                .ok_or_else(bad)?,
        ),
        Key::Mic if value == "off" => Setting::Mic(None),
        Key::Mic if value.trim().is_empty() => return Err(bad()),
        Key::Mic => Setting::Mic(Some(value.to_owned())),
        Key::Audio => Setting::Audio(on_off(value).ok_or_else(bad)?),
        Key::HideHud => Setting::HideHud(on_off(value).ok_or_else(bad)?),
        Key::AutoUpdates => Setting::AutoUpdates(on_off(value).ok_or_else(bad)?),
        Key::Beta => Setting::Beta(on_off(value).ok_or_else(bad)?),
        Key::Countdown => Setting::Countdown(
            value
                .parse::<u32>()
                .ok()
                .and_then(|n| Countdown::try_from(n).ok())
                .ok_or_else(bad)?,
        ),
        Key::Model => Setting::Model(parse_model(value).map_err(|_| bad())?),
        Key::Language => Setting::Language(match value {
            "auto" => Language::Auto,
            "ru" => Language::Russian,
            "en" => Language::English,
            _ => return Err(bad()),
        }),
        Key::Tracks => Setting::Tracks(match value {
            "mic" => Listen::Mic,
            "system" => Listen::System,
            "both" => Listen::Both,
            _ => return Err(bad()),
        }),
    };
    Ok(setting)
}

fn lines(settings: &AppSettings, keys: &[Key]) -> (Vec<String>, Map<String, Value>) {
    let mut lines = Vec::new();
    let mut json = Map::new();
    for key in keys {
        let value = text(&key.get(settings));
        lines.push(format!("{}={}", key.name(), argument(&value)));
        json.insert(key.name().to_owned(), value.into());
    }
    (lines, json)
}

/// `config show`: every key with its value.
pub(crate) fn show(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let file = ctx.app_settings_file()?;
    let settings = ctx.app_settings()?;
    let (lines, values) = lines(&settings, &Key::ALL);
    let defaults = AppSettings::default();
    let changed: Vec<&str> = Key::ALL
        .into_iter()
        .filter(|k| k.get(&settings) != k.get(&defaults))
        .map(Key::name)
        .collect();
    let json = json!({"file": file, "settings": values, "changed": changed});
    Ok(Answer::new(lines.join("\n"), json))
}

/// `config set key=value …`: all at once, or nothing when one is wrong.
pub(crate) fn set(ctx: &Ctx<'_>, assignments: &[String]) -> Result<Answer, CliError> {
    let values = assignments
        .iter()
        .map(|a| parse(a, ctx.cwd))
        .collect::<Result<Vec<_>, _>>()?;
    changed(ctx, Edit::Set(values))
}

/// `config reset [key…]`: these keys, or all of them, to their defaults.
pub(crate) fn reset(ctx: &Ctx<'_>, names: &[String]) -> Result<Answer, CliError> {
    let keys = names
        .iter()
        .map(|name| Key::find(name).ok_or_else(|| unknown(name)))
        .collect::<Result<Vec<_>, _>>()?;
    changed(ctx, Edit::Reset(keys))
}

fn changed(ctx: &Ctx<'_>, edit: Edit) -> Result<Answer, CliError> {
    let (applied, by) = app_settings::apply(ctx, edit)?;
    let (lines, values) = lines(&applied.settings, &applied.keys);
    let mut json = by.json();
    json["changed"] = Value::Object(values);
    Ok(Answer::new(app_settings::changed_text(by, lines), json))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
