//! Arguments of a command for `devcast help`, read from the op's JSON Schema or from clap.

use dc_core::OpInfo;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Arg {
    /// As typed: `<id>` for a positional argument, `--min-gap` for a named one.
    pub name: String,
    /// Russian type: `число`, `текст`, `флаг`, `a|b|c` for a choice …
    pub kind: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub about: String,
}

/// Positional arguments first, in order, then the named ones.
pub fn op_args(op: &OpInfo) -> Vec<Arg> {
    let schema = (op.schema)();
    let schema = schema.as_value();
    let empty = Map::new();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let required = |name: &str| {
        schema
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|r| r == name))
    };
    let positional = op.positional.iter().copied();
    let named = properties
        .keys()
        .map(String::as_str)
        .filter(|name| !op.positional.contains(name));
    positional
        .chain(named)
        .filter_map(|name| {
            let property = properties.get(name)?;
            Some(Arg {
                name: match op.positional.contains(&name) {
                    true => format!("<{name}>"),
                    false => format!("--{}", name.replace('_', "-")),
                },
                kind: kind(property, defs),
                required: required(name),
                default: property.get("default").map(Value::to_string),
                about: about(property, defs),
            })
        })
        .collect()
}

pub fn clap_args(command: &clap::Command) -> Vec<Arg> {
    command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .map(|arg| Arg {
            name: match arg.get_long() {
                Some(long) => format!("--{long}"),
                None => format!("<{}>", arg.get_id()),
            },
            kind: match arg.get_action().takes_values() {
                true => "текст".to_owned(),
                false => "флаг".to_owned(),
            },
            required: arg.is_required_set(),
            default: arg
                .get_default_values()
                .first()
                .map(|v| v.to_string_lossy().into_owned()),
            about: arg.get_help().map(ToString::to_string).unwrap_or_default(),
        })
        .collect()
}

fn resolve<'a>(schema: &'a Value, defs: &'a Map<String, Value>) -> &'a Value {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/$defs/"))
        .and_then(|name| defs.get(name))
        .unwrap_or(schema)
}

fn kind(schema: &Value, defs: &Map<String, Value>) -> String {
    let schema = resolve(schema, defs);
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return choices(values.iter());
    }
    if let Some(Value::Array(variants)) = schema.get("oneOf").or_else(|| schema.get("anyOf")) {
        let consts: Vec<&Value> = variants
            .iter()
            .filter_map(|v| resolve(v, defs).get("const"))
            .collect();
        if consts.len() == variants.len() {
            return choices(consts.into_iter());
        }
        let kinds: Vec<String> = variants
            .iter()
            .map(|v| kind(v, defs))
            .filter(|k| k != "null")
            .collect();
        return kinds.join(" или ");
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        let prefix: String = pattern
            .trim_start_matches('^')
            .chars()
            .take_while(char::is_ascii_lowercase)
            .collect();
        if !prefix.is_empty() {
            return format!("id {prefix}<число>");
        }
    }
    let types: Vec<&str> = match schema.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let names: Vec<&str> = types
        .into_iter()
        .filter(|t| *t != "null")
        .map(|t| match t {
            "string" => "текст",
            "number" => "число",
            "integer" => "целое",
            "boolean" => "флаг",
            "array" => "список",
            "object" => "объект",
            other => other,
        })
        .collect();
    match names.as_slice() {
        [] => "значение".to_owned(),
        _ => names.join(" или "),
    }
}

fn choices<'a>(values: impl Iterator<Item = &'a Value>) -> String {
    let names: Vec<String> = values
        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
        .collect();
    names.join("|")
}

fn about(schema: &Value, defs: &Map<String, Value>) -> String {
    [schema, resolve(schema, defs)]
        .into_iter()
        .find_map(|s| s.get("description").and_then(Value::as_str))
        .unwrap_or_default()
        .replace('\n', " ")
}
