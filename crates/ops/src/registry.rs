//! All ops of the app by name. Construction checks the rules every op must follow, so a broken op
//! fails the registry test instead of confusing an agent at run time.

use std::collections::BTreeSet;

use dc_core::{Command, EffectInfo, EffectRegistry, OpInfo};
use serde_json::{Map, Value};

use crate::error::OpsError;

/// Global flags of `devcast`, accepted anywhere on the command line; no op may use these fields.
pub const RESERVED_FIELDS: [&str; 3] = ["project", "json", "dry_run"];

#[derive(Debug, Clone)]
pub struct Registry {
    ops: Vec<OpInfo>,
    effects: EffectRegistry,
}

impl Registry {
    pub fn new(ops: Vec<OpInfo>, effects: Vec<EffectInfo>) -> Result<Self, String> {
        let effects = EffectRegistry::new(effects)?;
        let mut names = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut problems = Vec::new();
        for op in &ops {
            if !names.insert(op.name) {
                problems.push(format!("команда «{}» зарегистрирована дважды", op.name));
            }
            if let Some(key) = op.settings.filter(|key| !keys.insert(*key)) {
                problems.push(format!("{}: ключ настроек «{key}» уже занят", op.name));
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

    /// Keys of `devcast set <key>.<arg>=<value>` in registration order ([`dc_core::Op::SETTINGS`]).
    pub fn settings_keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.ops.iter().filter_map(|op| op.settings)
    }

    /// The op that changes the settings named `key` in `devcast set`.
    pub fn settings_op(&self, key: &str) -> Result<&OpInfo, OpsError> {
        self.ops
            .iter()
            .find(|op| op.settings == Some(key))
            .ok_or_else(|| {
                let keys: Vec<&str> = self.settings_keys().collect();
                OpsError::Usage(format!(
                    "нет настроек «{key}»: есть {}; чьи это команды и аргументы — devcast help set",
                    keys.join(", ")
                ))
            })
    }

    pub fn find(&self, name: &str) -> Result<&OpInfo, OpsError> {
        self.get(name)
            .ok_or_else(|| self.unknown(name, &[name.to_owned()]))
    }

    /// The op named by the first words of a command line (`zoom add --at 12` → `zoom add`) and
    /// how many words its name took. The longest matching name wins.
    pub fn resolve(&self, words: &[String]) -> Result<(&OpInfo, usize), OpsError> {
        let longest = self.ops.iter().map(|op| op.name.split(' ').count()).max();
        // A name never takes a flag: `zoom ad --at 1` is the unknown «zoom ad».
        let named = words.iter().take_while(|w| !w.starts_with("--")).count();
        let longest = longest.unwrap_or(1).min(named.max(1)).min(words.len());
        let prefixes: Vec<String> = (1..=longest).map(|n| words[..n].join(" ")).collect();
        for (n, name) in prefixes.iter().enumerate().rev() {
            if let Some(op) = self.get(name) {
                return Ok((op, n + 1));
            }
        }
        let typed = prefixes.last().cloned().unwrap_or_default();
        Err(self.unknown(&typed, &prefixes))
    }

    fn get(&self, name: &str) -> Option<&OpInfo> {
        self.ops.iter().find(|op| op.name == name)
    }

    /// «нет команды», with the op closest to any of the `typed` spellings.
    fn unknown(&self, name: &str, typed: &[String]) -> OpsError {
        let closest = self
            .ops
            .iter()
            .flat_map(|op| typed.iter().map(|t| (distance(op.name, t), op.name)))
            .filter(|(d, _)| *d <= 3)
            .min();
        OpsError::Usage(match closest {
            Some((_, similar)) => {
                format!("нет команды «{name}»; похоже, нужна «{similar}». Список: devcast help")
            }
            None => format!("нет команды «{name}». Список: devcast help"),
        })
    }

    pub fn command(
        &self,
        name: &str,
        args: Map<String, Value>,
    ) -> Result<Box<dyn Command>, OpsError> {
        let op = self.find(name)?;
        (op.parse)(Value::Object(args)).map_err(|e| {
            OpsError::Usage(format!("{name}: неверные аргументы ({e}). Пример: devcast {}; все поля: devcast help {name}", op.example))
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
    if op
        .settings
        .is_some_and(|key| key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase()))
    {
        problems.push("ключ настроек — одно слово из строчных латинских букв".to_owned());
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
    for field in RESERVED_FIELDS {
        if properties.is_some_and(|p| p.contains_key(field)) {
            problems.push(format!(
                "поле «{field}» занято глобальным флагом devcast, переименуй его"
            ));
        }
    }
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
