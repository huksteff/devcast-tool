//! `devcast defaults show|set|from-project|reset`: the settings new projects and recordings start
//! with (`dc_project::Defaults`, the file `defaults.json` of the app settings folder). `set` runs
//! the same settings ops as `devcast set` on a project without clips that holds the defaults, so
//! every value passes the effect's own checks.

use std::collections::BTreeMap;

use dc_core::{Command, History, Project};
use dc_project::Defaults;
use serde_json::{Map, Value, json};

use crate::edit::{Ctx, applied_json};
use crate::error::CliError;
use crate::run::Answer;

/// `defaults show`: the defaults that differ from the factory ones, a line per argument in the form
/// `devcast set` and `defaults set` take (`zoom.zoom_in=1.522575`), so a line can be copied there.
pub(crate) fn show(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let file = ctx.defaults_file()?;
    let defaults = Defaults::load(&file)?;
    let mut project = Project::default();
    defaults.apply(&mut project);
    let factory: BTreeMap<String, Value> =
        assignments(ctx, &Project::default())?.into_iter().collect();
    let changed: Vec<(String, Value)> = assignments(ctx, &project)?
        .into_iter()
        .filter(|(key, value)| factory.get(key) != Some(value))
        .collect();
    let mut lines: Vec<String> = changed
        .iter()
        .map(|(key, value)| format!("{key}={}", argument(value)))
        .collect();
    lines.push(match changed.is_empty() {
        true => "все настройки по умолчанию заводские".to_owned(),
        false => "остальные — заводские".to_owned(),
    });
    let changed: Map<String, Value> = changed.into_iter().collect();
    let json = json!({"file": file, "changed": changed, "settings": settings(ctx, &project)?});
    Ok(Answer::new(lines.join("\n"), json))
}

/// `key.arg` → value of every settings argument of every effect, in registration order.
fn assignments(ctx: &Ctx<'_>, project: &Project) -> Result<Vec<(String, Value)>, CliError> {
    let mut all = Vec::new();
    for effect in ctx.registry.effects().iter() {
        for op in (effect.settings_args)(project).map_err(broken)? {
            for (arg, value) in op.args {
                all.push((format!("{}.{arg}", op.key), value));
            }
        }
    }
    Ok(all)
}

/// A value as written after `=` on the command line; text with spaces or empty is quoted.
fn argument(value: &Value) -> String {
    match value {
        Value::String(text) if text.is_empty() || text.contains(char::is_whitespace) => {
            format!("\"{text}\"")
        }
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn broken(error: dc_core::StateError) -> CliError {
    CliError::Failed(format!(
        "{error}; файл настроек по умолчанию: devcast defaults reset вернёт заводские"
    ))
}

/// Effect id → its settings with every default filled in.
fn settings(ctx: &Ctx<'_>, project: &Project) -> Result<BTreeMap<String, Value>, CliError> {
    let mut all = BTreeMap::new();
    for effect in ctx.registry.effects().iter() {
        if let Some(value) = (effect.settings)(project).map_err(broken)? {
            all.insert(effect.id.to_owned(), value);
        }
    }
    Ok(all)
}

/// `defaults set zoom.preset=smooth`: like `devcast set`, into the defaults file.
pub(crate) fn set(ctx: &Ctx<'_>, assignments: &[String]) -> Result<Answer, CliError> {
    let commands = dc_ops::settings_commands(ctx.registry, assignments)?;
    let commands: Vec<&dyn Command> = commands.iter().map(AsRef::as_ref).collect();
    let file = ctx.defaults_file()?;
    let mut project = Project::default();
    Defaults::load(&file)?.apply(&mut project);
    let dry_run = ctx.globals.dry_run;
    let applied = dc_ops::apply_commands(
        &mut project,
        &mut History::default(),
        "defaults set",
        &commands,
        dry_run,
    )?;
    if applied.changed && !dry_run {
        Defaults::from_project(&project).save(&file)?;
    }
    let what = match applied.changed {
        true => applied.diff.to_string(),
        false => "без изменений".to_owned(),
    };
    let text = format!("{}по умолчанию: {what}", prefix(ctx));
    let mut json = applied_json(&applied);
    json["file"] = json!(file);
    Ok(Answer::new(text, json))
}

/// `defaults from-project`: the settings of the project become the defaults.
pub(crate) fn from_project(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let file = ctx.defaults_file()?;
    let workspace = ctx.open(false)?;
    let defaults = Defaults::from_project(&workspace.project);
    let effects: Vec<&str> = defaults.all().keys().map(String::as_str).collect();
    if !ctx.globals.dry_run {
        defaults.save(&file)?;
    }
    let text = format!(
        "{}по умолчанию — настройки «{}»: {}",
        prefix(ctx),
        workspace.dir().name(),
        match effects.is_empty() {
            true => "все заводские".to_owned(),
            false => format!("{}, остальные заводские", effects.join(", ")),
        }
    );
    let json = json!({"dry_run": ctx.globals.dry_run, "file": file, "effects": effects});
    Ok(Answer {
        warnings: workspace.warnings,
        ..Answer::new(text, json)
    })
}

/// `defaults reset`: back to the factory settings.
pub(crate) fn reset(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let file = ctx.defaults_file()?;
    if !ctx.globals.dry_run {
        Defaults::reset(&file)?;
    }
    let text = format!("{}по умолчанию — заводские настройки", prefix(ctx));
    let json = json!({"dry_run": ctx.globals.dry_run, "file": file});
    Ok(Answer::new(text, json))
}

fn prefix(ctx: &Ctx<'_>) -> &'static str {
    match ctx.globals.dry_run {
        true => "dry-run, файл не изменён: ",
        false => "",
    }
}
