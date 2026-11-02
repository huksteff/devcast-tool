//! `devcast hotkeys ls|set|reset`: the global hotkeys of recording and the editor shortcuts of
//! the table `dc_settings::keymap`, by id. The rules are the settings window's: a global one
//! needs Ctrl or Alt (no Win), a shortcut another command has is refused or, with `--swap`,
//! swapped («Заменить»). Applied by the app or in the file (`app_settings.rs`).

use dc_settings::{AppSettings, Binding, Edit, Shortcut};
use serde_json::{Value, json};

use crate::app_settings;
use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;

/// A cleared shortcut in the text.
const NONE: &str = "нет";

fn scope(binding: Binding) -> &'static str {
    match binding {
        Binding::Global(_) => "global",
        Binding::Editor(_) => "editor",
    }
}

fn shown(keys: Option<&str>) -> &str {
    keys.unwrap_or(NONE)
}

fn line(settings: &AppSettings, binding: Binding) -> String {
    let keys = binding.keys(settings);
    let default = binding.default_keys();
    let mut line = format!("{:<16} {}", binding.id(), shown(keys.as_deref()));
    if keys != default {
        line.push_str(&format!(" (по умолчанию {})", shown(default.as_deref())));
    }
    line
}

/// `hotkeys ls`: every shortcut with its id, the global ones first.
pub(crate) fn ls(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let settings = ctx.app_settings()?;
    let (global, editor): (Vec<Binding>, Vec<Binding>) =
        Binding::all().partition(|b| matches!(b, Binding::Global(_)));
    let mut lines = vec!["глобальные (из любой программы, нужен Ctrl или Alt):".to_owned()];
    lines.extend(global.iter().map(|b| line(&settings, *b)));
    lines.push("редактор:".to_owned());
    lines.extend(editor.iter().map(|b| line(&settings, *b)));
    let items: Vec<Value> = Binding::all()
        .map(|b| {
            json!({
                "id": b.id(),
                "scope": scope(b),
                "keys": b.keys(&settings),
                "default": b.default_keys(),
            })
        })
        .collect();
    Ok(Answer::new(lines.join("\n"), Value::Array(items)))
}

fn binding(id: &str) -> Result<Binding, CliError> {
    Binding::find(id).ok_or_else(|| {
        CliError::Usage(format!(
            "сочетания «{id}» нет: id — первый столбец devcast hotkeys ls (start_stop, save …)"
        ))
    })
}

/// `Ctrl+Shift+R`; modifiers in any order and case.
fn shortcut(text: &str) -> Result<Shortcut, CliError> {
    Shortcut::parse(text).ok_or_else(|| {
        CliError::Usage(format!(
            "сочетание «{text}» не читается: модификаторы Ctrl, Alt, Shift (Win нельзя) и клавиша, например Ctrl+Shift+R, F9, Delete"
        ))
    })
}

/// `hotkeys set <id> <keys> [--swap]` or `--clear`.
pub(crate) fn set(
    ctx: &Ctx<'_>,
    id: &str,
    keys: Option<&str>,
    (clear, swap): (bool, bool),
) -> Result<Answer, CliError> {
    let binding = binding(id)?;
    let keys = match (keys, clear) {
        (Some(keys), false) => Some(shortcut(keys)?),
        (None, true) => None,
        _ => {
            return Err(CliError::Usage(
                "нужно сочетание или --clear: devcast hotkeys set save Ctrl+S".to_owned(),
            ));
        }
    };
    changed(
        ctx,
        Edit::Bind {
            binding,
            keys,
            swap,
        },
    )
}

/// `hotkeys reset <id…>` or `--all`.
pub(crate) fn reset(ctx: &Ctx<'_>, ids: &[String], all: bool) -> Result<Answer, CliError> {
    if ids.is_empty() != all {
        return Err(CliError::Usage(
            "нужны id сочетаний или --all: devcast hotkeys reset save".to_owned(),
        ));
    }
    let bindings = ids
        .iter()
        .map(|id| binding(id))
        .collect::<Result<Vec<_>, _>>()?;
    changed(ctx, Edit::ResetKeys(bindings))
}

fn changed(ctx: &Ctx<'_>, edit: Edit) -> Result<Answer, CliError> {
    let (applied, by) = app_settings::apply(ctx, edit)?;
    let mut lines = Vec::new();
    let mut values = serde_json::Map::new();
    for binding in &applied.bindings {
        let keys = binding.keys(&applied.settings);
        lines.push(format!("{}={}", binding.id(), shown(keys.as_deref())));
        values.insert(binding.id().to_owned(), json!(keys));
    }
    let mut json = by.json();
    json["changed"] = Value::Object(values);
    Ok(Answer::new(app_settings::changed_text(by, lines), json))
}

#[cfg(test)]
#[path = "hotkeys_tests.rs"]
mod tests;
