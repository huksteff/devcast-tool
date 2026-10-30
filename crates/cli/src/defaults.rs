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

/// `defaults show`: the defaults that differ from the factory ones, a field per line.
pub(crate) fn show(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let file = ctx.defaults_file()?;
    let defaults = Defaults::load(&file)?;
    let mut project = Project::default();
    defaults.apply(&mut project);
    let current = settings(ctx, &project)?;
    let factory = settings(ctx, &Project::default())?;
    let mut changed = Map::new();
    for (id, value) in &current {
        let mut now = BTreeMap::new();
        let mut then = BTreeMap::new();
        flatten(id, value, &mut now);
        if let Some(value) = factory.get(id) {
            flatten(id, value, &mut then);
        }
        for (key, value) in now {
            if then.get(&key) != Some(&value) {
                changed.insert(key, value);
            }
        }
    }
    let mut lines: Vec<String> = changed
        .iter()
        .map(|(key, value)| match value {
            Value::String(text) => format!("{key}={text}"),
            other => format!("{key}={other}"),
        })
        .collect();
    lines.push(match changed.is_empty() {
        true => "все настройки по умолчанию заводские".to_owned(),
        false => "остальные — заводские".to_owned(),
    });
    let json = json!({"file": file, "changed": changed, "settings": current});
    Ok(Answer::new(lines.join("\n"), json))
}

/// Effect id → its settings with every default filled in.
fn settings(ctx: &Ctx<'_>, project: &Project) -> Result<BTreeMap<String, Value>, CliError> {
    let mut all = BTreeMap::new();
    for effect in ctx.registry.effects().iter() {
        let value = (effect.settings)(project).map_err(|e| {
            CliError::Failed(format!(
                "{e}; файл настроек по умолчанию: devcast defaults reset вернёт заводские"
            ))
        })?;
        if let Some(value) = value {
            all.insert(effect.id.to_owned(), value);
        }
    }
    Ok(all)
}

fn flatten(key: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(fields) => {
            for (inner, value) in fields {
                flatten(&format!("{key}.{inner}"), value, out);
            }
        }
        other => {
            out.insert(key.to_owned(), other.clone());
        }
    }
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
