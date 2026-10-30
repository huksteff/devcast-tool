//! App defaults: the effect settings new projects start with. Stored as a project file without
//! clips whose effect states hold only their `settings` field, so the schema migrations apply to
//! it unchanged. No file = factory defaults (every effect's `Default`).

use std::collections::BTreeMap;
use std::path::Path;

use dc_core::Project;
use dc_core::effect::SETTINGS_FIELD;
use serde_json::Value;

use crate::atomic;
use crate::dir::read_project;
use crate::error::{ProjectError, Result};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Defaults {
    /// Effect id → its `settings` value.
    settings: BTreeMap<String, Value>,
}

impl Defaults {
    pub fn factory() -> Self {
        Self::default()
    }

    /// Reads the defaults file; a missing file means factory defaults.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::factory());
        }
        read_project(path).map(|project| Self::from_project(&project))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ProjectError::io("создать папку", parent))?;
        }
        let mut project = Project::default();
        self.apply(&mut project);
        atomic::write_json(path, &project)
    }

    /// «Сбросить к заводским».
    pub fn reset(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(ProjectError::io("удалить", path)(e))
            }
            _ => Ok(()),
        }
    }

    /// «Сохранить текущие как по умолчанию»: the settings of every effect in `project`.
    pub fn from_project(project: &Project) -> Self {
        let settings = project
            .effects
            .iter()
            .filter_map(|(id, state)| Some((id.clone(), state.get(SETTINGS_FIELD)?.clone())))
            .collect();
        Self { settings }
    }

    /// Puts the default settings into `project` (a new project or recording), keeping its content.
    pub fn apply(&self, project: &mut Project) {
        for (id, value) in &self.settings {
            let state = project
                .effects
                .entry(id.clone())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(object) = state.as_object_mut() {
                object.insert(SETTINGS_FIELD.to_owned(), value.clone());
            }
        }
    }

    /// Makes these the settings of `project` («копировать стиль»): an effect without settings here
    /// goes back to its factory settings, content stays. A state left empty is removed, as a
    /// state equal to `Default` is.
    pub fn restyle(&self, project: &mut Project) {
        project.effects.retain(|id, state| {
            if !self.settings.contains_key(id)
                && let Some(object) = state.as_object_mut()
            {
                object.remove(SETTINGS_FIELD);
                return !object.is_empty();
            }
            true
        });
        self.apply(project);
    }

    /// Effect id → its `settings` value, as stored.
    pub fn all(&self) -> &BTreeMap<String, Value> {
        &self.settings
    }

    /// Default settings of one effect, for the «Сбросить» button of its settings group;
    /// `None` means the effect's factory settings.
    pub fn settings(&self, effect: &str) -> Option<&Value> {
        self.settings.get(effect)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn project_with(effects: Value) -> Project {
        Project {
            effects: serde_json::from_value(effects).unwrap(),
            ..Project::default()
        }
    }

    #[test]
    fn copies_settings_but_not_content() {
        let current = project_with(json!({
            "zoom": {"settings": {"smoothness": 0.7}, "regions": [{"id": "z1"}]},
            "captions": {"words": ["привет"]},
        }));
        let defaults = Defaults::from_project(&current);
        let mut fresh = Project::default();
        defaults.apply(&mut fresh);
        assert_eq!(
            fresh,
            project_with(json!({"zoom": {"settings": {"smoothness": 0.7}}}))
        );
        assert_eq!(defaults.settings("zoom"), Some(&json!({"smoothness": 0.7})));
        assert_eq!(defaults.settings("captions"), None);
    }

    #[test]
    fn apply_keeps_existing_content() {
        let defaults = Defaults::from_project(&project_with(json!({"zoom": {"settings": 1}})));
        let mut recording = project_with(json!({"zoom": {"regions": [1, 2]}}));
        defaults.apply(&mut recording);
        assert_eq!(
            recording,
            project_with(json!({"zoom": {"regions": [1, 2], "settings": 1}}))
        );
    }

    #[test]
    fn restyle_replaces_settings_and_keeps_content() {
        let style = Defaults::from_project(&project_with(json!({
            "zoom": {"settings": {"smoothness": 0.7}},
        })));
        let mut target = project_with(json!({
            "zoom": {"settings": {"smoothness": 0.1}, "zooms": [1]},
            "cursor": {"settings": {"size": 3}},
            "captions": {"settings": {"size": 40}, "cues": [2]},
        }));
        style.restyle(&mut target);
        assert_eq!(
            target,
            project_with(json!({
                "zoom": {"settings": {"smoothness": 0.7}, "zooms": [1]},
                "captions": {"cues": [2]},
            }))
        );
    }

    #[test]
    fn file_round_trip_and_reset_to_factory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Настройки DevCast").join("defaults.json");
        assert_eq!(Defaults::load(&path).unwrap(), Defaults::factory());
        let defaults = Defaults::from_project(&project_with(json!({"crop": {"settings": "16:9"}})));
        defaults.save(&path).unwrap();
        assert_eq!(Defaults::load(&path).unwrap(), defaults);
        Defaults::reset(&path).unwrap();
        Defaults::reset(&path).unwrap();
        assert_eq!(Defaults::load(&path).unwrap(), Defaults::factory());
    }
}
