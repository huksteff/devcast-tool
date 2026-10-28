//! All ops of the app by name. Construction checks the rules every op must follow, so a broken op
//! fails the registry test instead of confusing an agent at run time.

use std::collections::BTreeSet;

use dc_core::{Command, EffectInfo, EffectRegistry, OpInfo};
use serde_json::{Map, Value};

use crate::error::OpsError;

#[derive(Debug, Clone)]
pub struct Registry {
    ops: Vec<OpInfo>,
    effects: EffectRegistry,
}

impl Registry {
    pub fn new(ops: Vec<OpInfo>, effects: Vec<EffectInfo>) -> Result<Self, String> {
        let effects = EffectRegistry::new(effects)?;
        let mut names = BTreeSet::new();
        let mut problems = Vec::new();
        for op in &ops {
            if !names.insert(op.name) {
                problems.push(format!("команда «{}» зарегистрирована дважды", op.name));
            }
            problems.extend(check(op).into_iter().map(|p| format!("{}: {p}", op.name)));
        }
        if problems.is_empty() {
            Ok(Self { ops, effects })
        } else {
            Err(problems.join("\n"))
        }
    }

    /// Ops in registration order (grouped by effect).
    pub fn ops(&self) -> &[OpInfo] {
        &self.ops
    }

    pub fn effects(&self) -> &EffectRegistry {
        &self.effects
    }

    pub fn find(&self, name: &str) -> Result<&OpInfo, OpsError> {
        if let Some(op) = self.ops.iter().find(|op| op.name == name) {
            return Ok(op);
        }
        let closest = self
            .ops
            .iter()
            .map(|op| (distance(op.name, name), op.name))
            .filter(|(d, _)| *d <= 3)
            .min();
        Err(OpsError(match closest {
            Some((_, similar)) => {
                format!("нет команды «{name}»; похоже, нужна «{similar}». Список: devcast help")
            }
            None => format!("нет команды «{name}». Список: devcast help"),
        }))
    }

    /// Builds the command `name` from its arguments.
    pub fn command(
        &self,
        name: &str,
        args: Map<String, Value>,
    ) -> Result<Box<dyn Command>, OpsError> {
        let op = self.find(name)?;
        (op.parse)(Value::Object(args)).map_err(|e| {
            OpsError(format!("{name}: неверные аргументы ({e}). Пример: devcast {}; все поля: devcast help {name}", op.example))
        })
    }
}

/// Rules every op follows; violations are bugs in the op.
fn check(op: &OpInfo) -> Vec<String> {
    let mut problems = Vec::new();
    let words: Vec<&str> = op.name.split(' ').collect();
    if words
        .iter()
        .any(|w| w.is_empty() || !w.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
    {
        problems.push("имя — слова из строчных латинских букв через пробел".to_owned());
    }
    if op.about.is_empty() || op.about.contains('\n') {
        problems.push("описание — одна непустая строка".to_owned());
    }
    if !op.example.starts_with(op.name) {
        problems.push(format!(
            "пример «{}» должен начинаться с имени команды",
            op.example
        ));
    }
    let schema = (op.schema)();
    if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
        problems.push(
            "нужен #[serde(deny_unknown_fields)], иначе опечатка в аргументе молча игнорируется"
                .to_owned(),
        );
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    for name in op.positional {
        if !properties.is_some_and(|p| p.contains_key(*name)) {
            problems.push(format!("позиционного аргумента «{name}» нет среди полей"));
        }
    }
    problems
}

/// Levenshtein distance by characters, for «похоже, нужна …».
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != *cb));
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
