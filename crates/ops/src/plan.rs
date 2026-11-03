//! Steps of a change: one from the command line, or many from a plan file
//! `[{"op": "zoom add", "at": 12.5}, {"op": "cut", "range": "10-12"}]`.

pub use dc_core::Step;
use serde_json::Value;

use crate::error::OpsError;
use crate::registry::Registry;

const PLAN_HINT: &str = r#"план — JSON-массив шагов вида {"op": "zoom add", "at": 12.5}"#;

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

/// Commands of a step, as [`build_with`] takes them from its caller.
pub type Commands = Vec<Box<dyn dc_core::Command>>;

/// Builds the commands of all steps, failing on the first bad one with its number.
pub(crate) fn build(registry: &Registry, steps: &[Step]) -> Result<Commands, OpsError> {
    build_with(registry, steps, |_| Ok::<_, OpsError>(None))
}

/// Like [`build`], but `expand` builds a step first when it is a shorthand only the caller can
/// resolve (`cut w12-w20`, `zoom add --at clicks` of the CLI); `None` leaves it to the registry.
pub fn build_with<E: From<OpsError> + std::fmt::Display>(
    registry: &Registry,
    steps: &[Step],
    mut expand: impl FnMut(&Step) -> Result<Option<Commands>, E>,
) -> Result<Commands, E> {
    let mut commands = Commands::new();
    for (index, step) in steps.iter().enumerate() {
        let built = match expand(step) {
            Ok(Some(built)) => Ok(built),
            Ok(None) => registry
                .command(&step.op, step.args.clone())
                .map(|command| vec![command])
                .map_err(E::from),
            Err(e) => Err(e),
        };
        commands.extend(built.map_err(|e| match steps.len() {
            1 => e,
            _ => E::from(OpsError::Usage(format!("шаг {}: {e}", index + 1))),
        })?);
    }
    Ok(commands)
}

/// History name of `steps`: the op of a single step, `apply` for a plan.
pub fn step_name(steps: &[Step]) -> &str {
    match steps {
        [single] => single.op.as_str(),
        _ => "apply",
    }
}
