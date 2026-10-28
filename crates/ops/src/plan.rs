//! Steps of a change: one from the command line, or many from a plan file
//! `[{"op": "zoom add", "at": 12.5}, {"op": "cut", "range": "10-12"}]`.

use serde_json::{Map, Value};

use crate::error::OpsError;
use crate::registry::Registry;

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub op: String,
    pub args: Map<String, Value>,
}

impl Step {
    pub fn new(op: impl Into<String>, args: Map<String, Value>) -> Self {
        Self {
            op: op.into(),
            args,
        }
    }
}

const PLAN_HINT: &str = r#"план — JSON-массив шагов вида {"op": "zoom add", "at": 12.5}"#;

/// Parses a plan file.
pub fn parse_plan(text: &str) -> Result<Vec<Step>, OpsError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| OpsError::Usage(format!("план не читается как JSON ({e}); {PLAN_HINT}")))?;
    let Value::Array(items) = value else {
        return Err(OpsError::Usage(format!(
            "{PLAN_HINT}, а не одиночный объект"
        )));
    };
    if items.is_empty() {
        return Err(OpsError::Usage(format!("план пуст; {PLAN_HINT}")));
    }
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let Value::Object(mut args) = item else {
                return Err(OpsError::Usage(format!("шаг {}: {PLAN_HINT}", index + 1)));
            };
            match args.remove("op") {
                Some(Value::String(op)) => Ok(Step { op, args }),
                _ => Err(OpsError::Usage(format!(
                    "шаг {}: нет поля \"op\" с именем команды; {PLAN_HINT}",
                    index + 1
                ))),
            }
        })
        .collect()
}

/// Builds the commands of all steps, failing on the first bad one with its number.
pub(crate) fn build(
    registry: &Registry,
    steps: &[Step],
) -> Result<Vec<Box<dyn dc_core::Command>>, OpsError> {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            registry
                .command(&step.op, step.args.clone())
                .map_err(|e| match steps.len() {
                    1 => e,
                    _ => OpsError::Usage(format!("шаг {}: {e}", index + 1)),
                })
        })
        .collect()
}
