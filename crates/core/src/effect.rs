//! Effects: each effect crate owns a typed state stored in [`Project::effects`] under its id.
//! [`EffectRegistry`] lists all effects and checks that ids and id prefixes do not clash.

use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::command::Op;
use crate::id::{IdKind, is_valid_prefix};
use crate::item::Item;
use crate::project::{ClipKind, Project};

/// Field of an effect state that holds its user settings (style, smoothness, format …), as opposed
/// to content (zoom regions, captions). App defaults copy only this field between projects, so an
/// effect with settings declares `#[serde(default)] settings: …Settings` in its state.
pub const SETTINGS_FIELD: &str = "settings";

/// An effect (zoom, cursor, captions, …). Implemented by a unit type in the effect's crate.
pub trait Effect: 'static {
    /// Key in `project.effects` and in CLI commands, e.g. `"zoom"`.
    const ID: &'static str;
    /// Russian name for the UI, e.g. `"Зум"`.
    const TITLE: &'static str;
    /// Everything the effect stores in the project. A missing key means `Default`.
    type State: Serialize
        + DeserializeOwned
        + JsonSchema
        + Default
        + Clone
        + PartialEq
        + fmt::Debug
        + Send
        + Sync;

    /// Lists of `devcast ls` this effect fills, e.g. `["zooms"]`.
    const LISTS: &'static [&'static str] = &[];

    /// Objects of the effect's track, for `devcast ls`.
    fn items(_state: &Self::State) -> Result<Vec<Item>, serde_json::Error> {
        Ok(Vec::new())
    }

    /// Settings of `state` as its settings ops filled with every argument (see [`SettingsArgs`]),
    /// for `devcast defaults show`; empty when no op has [`crate::Op::SETTINGS`] for it.
    fn settings_args(_state: &Self::State) -> Result<Vec<SettingsArgs>, serde_json::Error> {
        Ok(Vec::new())
    }
}

/// One settings op ([`crate::Op::SETTINGS`]) as `devcast set` takes it: the key and each argument
/// with its value as on the command line (`--arg value`). A line `key.arg=value` goes into `set`.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsArgs {
    pub key: &'static str,
    pub args: serde_json::Map<String, serde_json::Value>,
}

impl SettingsArgs {
    /// `op` with every argument filled in; arguments left `None` are skipped.
    pub fn of<O: Op + Serialize>(op: &O) -> Result<Self, serde_json::Error> {
        let Some(key) = O::SETTINGS else {
            return Err(serde::ser::Error::custom(format!(
                "команда «{}» не меняет настройки: у неё нет Op::SETTINGS",
                O::NAME
            )));
        };
        let serde_json::Value::Object(mut args) = serde_json::to_value(op)? else {
            return Err(serde::ser::Error::custom(format!(
                "аргументы команды «{}» — не объект",
                O::NAME
            )));
        };
        args.retain(|_, value| !value.is_null());
        Ok(Self { key, args })
    }
}

/// Effect state in the project does not match the effect's schema.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "состояние эффекта «{effect}» в проекте повреждено: {message}. Исправь project.json по схеме (`devcast help --json`) или отмени последнее изменение"
)]
pub struct StateError {
    effect: &'static str,
    message: String,
}

impl Project {
    /// Typed state of effect `E`; `Default` if the project has none yet.
    pub fn state<E: Effect>(&self) -> Result<E::State, StateError> {
        match self.effects.get(E::ID) {
            None => Ok(E::State::default()),
            Some(value) => serde_json::from_value(value.clone()).map_err(|e| StateError {
                effect: E::ID,
                message: e.to_string(),
            }),
        }
    }

    /// Stores the state of effect `E`. A state equal to `Default` is removed to keep the file small.
    pub fn set_state<E: Effect>(&mut self, state: &E::State) -> Result<(), StateError> {
        if *state == E::State::default() {
            self.effects.remove(E::ID);
            return Ok(());
        }
        let value = serde_json::to_value(state).map_err(|e| StateError {
            effect: E::ID,
            message: e.to_string(),
        })?;
        self.effects.insert(E::ID.to_owned(), value);
        Ok(())
    }
}

/// What the rest of the app needs to know about an effect without depending on its crate types.
#[derive(Clone)]
pub struct EffectInfo {
    pub id: &'static str,
    pub title: &'static str,
    /// JSON Schema of the state, for `devcast help --json` and validation.
    pub schema: fn() -> schemars::Schema,
    /// Prefixes of the id kinds this effect owns (e.g. `["z"]` for zoom regions).
    pub id_prefixes: Vec<&'static str>,
    /// See [`Effect::LISTS`].
    pub lists: &'static [&'static str],
    /// Objects of its track in `project`, see [`Effect::items`].
    pub items: fn(&Project) -> Result<Vec<Item>, StateError>,
    /// The [`SETTINGS_FIELD`] of its state in `project` with every default filled in (a missing
    /// state is `Default`); `None` when the state has no settings.
    pub settings: fn(&Project) -> Result<Option<serde_json::Value>, StateError>,
    /// See [`Effect::settings_args`].
    pub settings_args: fn(&Project) -> Result<Vec<SettingsArgs>, StateError>,
}

impl fmt::Debug for EffectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectInfo")
            .field("id", &self.id)
            .field("id_prefixes", &self.id_prefixes)
            .finish()
    }
}

impl EffectInfo {
    pub fn of<E: Effect>(id_prefixes: Vec<&'static str>) -> Self {
        Self {
            id: E::ID,
            title: E::TITLE,
            schema: || schemars::schema_for!(E::State),
            id_prefixes,
            lists: E::LISTS,
            items: |project| {
                let state = project.state::<E>()?;
                E::items(&state).map_err(|e| StateError {
                    effect: E::ID,
                    message: e.to_string(),
                })
            },
            settings: |project| {
                let state = project.state::<E>()?;
                let value = serde_json::to_value(&state).map_err(|e| StateError {
                    effect: E::ID,
                    message: e.to_string(),
                })?;
                Ok(value.get(SETTINGS_FIELD).cloned())
            },
            settings_args: |project| {
                let state = project.state::<E>()?;
                E::settings_args(&state).map_err(|e| StateError {
                    effect: E::ID,
                    message: e.to_string(),
                })
            },
        }
    }
}

/// All effects of the app. Construction fails if two effects share an id or an id prefix.
#[derive(Debug, Clone)]
pub struct EffectRegistry {
    effects: Vec<EffectInfo>,
}

impl EffectRegistry {
    pub fn new(effects: Vec<EffectInfo>) -> Result<Self, String> {
        let mut ids = BTreeSet::new();
        let mut prefixes: BTreeSet<&str> = BTreeSet::from([ClipKind::PREFIX]);
        let mut problems = Vec::new();
        for effect in &effects {
            if !ids.insert(effect.id) {
                problems.push(format!("эффект «{}» зарегистрирован дважды", effect.id));
            }
            for prefix in &effect.id_prefixes {
                if !is_valid_prefix(prefix) {
                    problems.push(format!(
                        "{}: префикс id «{prefix}» должен состоять из строчных латинских букв",
                        effect.id
                    ));
                } else if !prefixes.insert(prefix) {
                    problems.push(format!("{}: префикс id «{prefix}» уже занят", effect.id));
                }
            }
        }
        if problems.is_empty() {
            Ok(Self { effects })
        } else {
            Err(problems.join("\n"))
        }
    }

    pub fn get(&self, id: &str) -> Option<&EffectInfo> {
        self.effects.iter().find(|e| e.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &EffectInfo> {
        self.effects.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Default, Serialize, serde::Deserialize, JsonSchema)]
    struct Items {
        /// Stored numbers.
        items: Vec<u32>,
    }

    struct Test;
    impl Effect for Test {
        const ID: &'static str = "test";
        const TITLE: &'static str = "Тест";
        type State = Items;
    }

    #[test]
    fn missing_state_is_default_and_default_is_not_stored() {
        let mut p = Project::default();
        assert_eq!(p.state::<Test>().unwrap(), Items::default());
        p.set_state::<Test>(&Items { items: vec![1] }).unwrap();
        assert!(p.effects.contains_key("test"));
        p.set_state::<Test>(&Items::default()).unwrap();
        assert!(!p.effects.contains_key("test"));
    }

    #[test]
    fn broken_state_explains_how_to_fix() {
        let mut p = Project::default();
        p.effects
            .insert("test".to_owned(), serde_json::json!({"items": "oops"}));
        let error = p.state::<Test>().unwrap_err().to_string();
        assert!(
            error.contains("эффекта «test»") && error.contains("devcast help --json"),
            "{error}"
        );
    }

    #[test]
    fn registry_rejects_clashes() {
        let a = EffectInfo::of::<Test>(vec!["z"]);
        let b = EffectInfo::of::<Test>(vec!["c", "Z"]);
        let error = EffectRegistry::new(vec![a.clone(), b]).unwrap_err();
        assert!(error.contains("зарегистрирован дважды"), "{error}");
        assert!(error.contains("«c» уже занят"), "{error}");
        assert!(error.contains("«Z» должен состоять"), "{error}");
        assert!(EffectRegistry::new(vec![a]).is_ok());
    }

    #[test]
    fn schema_is_available_without_the_effect_type() {
        let info = EffectInfo::of::<Test>(vec![]);
        let json = serde_json::to_string(&(info.schema)()).unwrap();
        assert!(json.contains("items"), "{json}");
    }

    #[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize, JsonSchema)]
    #[serde(default)]
    struct Styled {
        /// Stored numbers.
        items: Vec<u32>,
        /// Settings of the test effect.
        settings: u32,
    }

    impl Default for Styled {
        fn default() -> Self {
            Self {
                items: Vec::new(),
                settings: 7,
            }
        }
    }

    struct WithSettings;
    impl Effect for WithSettings {
        const ID: &'static str = "styled";
        const TITLE: &'static str = "Со стилем";
        type State = Styled;
    }

    #[test]
    fn settings_are_read_with_defaults_filled_in() {
        let info = EffectInfo::of::<WithSettings>(vec![]);
        let mut p = Project::default();
        assert_eq!((info.settings)(&p).unwrap(), Some(serde_json::json!(7)));
        p.effects
            .insert("styled".to_owned(), serde_json::json!({"settings": 3}));
        assert_eq!((info.settings)(&p).unwrap(), Some(serde_json::json!(3)));
        let plain = EffectInfo::of::<Test>(vec![]);
        assert_eq!((plain.settings)(&p).unwrap(), None);
    }
}
