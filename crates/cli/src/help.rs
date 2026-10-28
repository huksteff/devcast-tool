//! `devcast help`: one line per command, one command in detail with an example, or everything as
//! JSON with the schemas of every op, of the project and of every effect state.
//! Built-in commands describe themselves through clap: `about` is the line, `after_help` the example.

use clap::CommandFactory;
use dc_core::{OpInfo, Project};
use dc_ops::Registry;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::help_args::{Arg, clap_args, op_args};
use crate::run::{Answer, Cli};

struct Entry<'a> {
    name: String,
    about: String,
    example: String,
    args: Vec<Arg>,
    op: Option<&'a OpInfo>,
}

pub(crate) fn help(topic: &[String], registry: &Registry) -> Result<Answer, CliError> {
    let entries = entries(registry);
    if topic.is_empty() {
        return Ok(Answer::new(
            list_text(&entries),
            list_json(&entries, registry),
        ));
    }
    let name = topic.join(" ");
    match entries.iter().find(|e| e.name == name) {
        Some(entry) => Ok(Answer::new(entry_text(entry), entry_json(entry))),
        None => {
            registry.resolve(topic)?;
            Err(CliError::Usage(format!(
                "нет команды «{name}». Список: devcast help"
            )))
        }
    }
}

fn entries(registry: &Registry) -> Vec<Entry<'_>> {
    let cli = Cli::command();
    let builtins = cli.get_subcommands().flat_map(|command| {
        let children: Vec<Entry<'_>> = command
            .get_subcommands()
            .map(|child| {
                builtin(
                    format!("{} {}", command.get_name(), child.get_name()),
                    child,
                )
            })
            .collect();
        match children.is_empty() {
            true => vec![builtin(command.get_name().to_owned(), command)],
            false => children,
        }
    });
    let ops = registry.ops().iter().map(|op| Entry {
        name: op.name.to_owned(),
        about: op.about.to_owned(),
        example: op.example.to_owned(),
        args: op_args(op),
        op: Some(op),
    });
    builtins.chain(ops).collect()
}

/// A built-in command; a group like `clip` is listed by its subcommands (`clip add`).
fn builtin(name: String, command: &clap::Command) -> Entry<'static> {
    Entry {
        name,
        about: command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        example: command
            .get_after_help()
            .map(ToString::to_string)
            .unwrap_or_default(),
        args: clap_args(command),
        op: None,
    }
}

fn list_text(entries: &[Entry<'_>]) -> String {
    let width = entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<String> = entries
        .iter()
        .map(|e| format!("{:width$}  {}", e.name, e.about))
        .collect();
    lines.push("Подробнее: devcast help <команда>; всё со схемами: devcast help --json".to_owned());
    lines.join("\n")
}

fn entry_text(entry: &Entry<'_>) -> String {
    let mut lines = vec![
        format!("{} — {}", entry.name, entry.about),
        format!("пример: devcast {}", entry.example),
    ];
    for arg in &entry.args {
        let mut line = format!("  {}: {}", arg.name, arg.kind);
        if arg.required {
            line.push_str(", обязательный");
        }
        if let Some(default) = &arg.default {
            line.push_str(&format!(", по умолчанию {default}"));
        }
        if !arg.about.is_empty() {
            line.push_str(&format!(" — {}", arg.about));
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn entry_json(entry: &Entry<'_>) -> Value {
    let mut value = json!({
        "name": entry.name,
        "about": entry.about,
        "example": format!("devcast {}", entry.example),
        "args": entry.args,
    });
    if let Some(op) = entry.op {
        value["schema"] = (op.schema)().to_value();
    }
    value
}

fn list_json(entries: &[Entry<'_>], registry: &Registry) -> Value {
    let effects: serde_json::Map<String, Value> = registry
        .effects()
        .iter()
        .map(|e| (e.id.to_owned(), (e.schema)().to_value()))
        .collect();
    json!({
        "commands": entries.iter().map(entry_json).collect::<Vec<_>>(),
        "project": schemars::schema_for!(Project).to_value(),
        "effects": effects,
    })
}
