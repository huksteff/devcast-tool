//! Command line arguments of one op → its JSON arguments. Positional values fill
//! [`OpInfo::positional`] in order; `--name value`, `--name=value` and a bare `--flag` (= true)
//! fill the rest; `--min-gap` is field `min_gap`. A value becomes JSON (number, bool, array) unless
//! the field is a string in the op's schema, so `--text 123` stays text.

use dc_core::OpInfo;
use serde_json::{Map, Value};

use crate::error::OpsError;

pub fn parse_args(op: &OpInfo, args: &[String]) -> Result<Map<String, Value>, OpsError> {
    let schema = (op.schema)();
    let properties = schema.get("properties").and_then(Value::as_object);
    let is_string = |field: &str| {
        properties
            .and_then(|p| p.get(field))
            .and_then(|s| s.get("type"))
            .is_some_and(|t| match t {
                Value::String(t) => t == "string",
                Value::Array(types) => {
                    types.contains(&Value::from("string"))
                        && !types.contains(&Value::from("number"))
                }
                _ => false,
            })
    };
    let mut fields = Map::new();
    let mut positional = op.positional.iter();
    let mut rest = args.iter().peekable();
    while let Some(arg) = rest.next() {
        let (field, raw) = if let Some(flag) = arg.strip_prefix("--").filter(|f| !f.is_empty()) {
            match flag.split_once('=') {
                Some((name, value)) => (name.replace('-', "_"), Some(value.to_owned())),
                None => {
                    let value = rest.next_if(|next| !next.starts_with("--")).cloned();
                    (flag.replace('-', "_"), value)
                }
            }
        } else {
            let Some(name) = positional.next() else {
                return Err(OpsError::Usage(format!(
                    "{}: лишний аргумент «{arg}». Пример: devcast {}",
                    op.name, op.example
                )));
            };
            ((*name).to_owned(), Some(arg.clone()))
        };
        let value = match raw {
            None => Value::Bool(true),
            Some(text) if is_string(&field) => Value::String(text),
            Some(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        };
        if fields.insert(field.clone(), value).is_some() {
            return Err(OpsError::Usage(format!(
                "{}: аргумент «{field}» указан дважды",
                op.name
            )));
        }
    }
    Ok(fields)
}
