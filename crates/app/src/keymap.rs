//! The table of editor shortcuts (roadmap 7.3): every editor action with its id (the key in
//! `settings.json`), name, group and default shortcut by physical key. The editor dispatches by
//! it (`editor::keys`), the help and the tooltips read it, the settings reassign it. Defaults
//! follow the mockup and the owner's decisions (A «Выбор», B «Лезвие», T annotation,
//! Shift+Delete); all of them can be changed.

use std::collections::BTreeMap;

use dc_ui_kit::strings::keys as text;
use serde::{Deserialize, Serialize};

use crate::hotkeys::Command;
use crate::settings::AppSettings;
use crate::shortcut::Shortcut;

/// Section of the help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Playback,
    Montage,
    Effects,
    Edit,
    Project,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Self::Playback => text::PLAYBACK,
            Self::Montage => text::MONTAGE,
            Self::Effects => text::EFFECTS,
            Self::Edit => text::EDIT,
            Self::Project => text::PROJECT,
        }
    }
}

/// An editor action that has a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorKey {
    PlayPause,
    PrevFrame,
    NextFrame,
    PrevKey,
    NextKey,
    CutAtPlayhead,
    Select,
    Blade,
    CutRange,
    Snap,
    AddZoom,
    AddAnnotation,
    Keyframe,
    NextAnnotation,
    Undo,
    Redo,
    RedoAlt,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Delete,
    NewProject,
    OpenProject,
    Save,
    SaveAs,
    Help,
}

/// Id, group, name and default shortcut of an action.
struct Spec(&'static str, Group, &'static str, &'static str);

impl EditorKey {
    pub const ALL: [Self; 27] = [
        Self::PlayPause,
        Self::PrevFrame,
        Self::NextFrame,
        Self::PrevKey,
        Self::NextKey,
        Self::CutAtPlayhead,
        Self::Select,
        Self::Blade,
        Self::CutRange,
        Self::Snap,
        Self::AddZoom,
        Self::AddAnnotation,
        Self::Keyframe,
        Self::NextAnnotation,
        Self::Undo,
        Self::Redo,
        Self::RedoAlt,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::SelectAll,
        Self::Delete,
        Self::NewProject,
        Self::OpenProject,
        Self::Save,
        Self::SaveAs,
        Self::Help,
    ];

    fn spec(self) -> Spec {
        use Group::{Edit, Effects, Montage, Playback, Project};
        match self {
            Self::PlayPause => Spec("play_pause", Playback, text::PLAY_PAUSE, "Space"),
            Self::PrevFrame => Spec("prev_frame", Playback, text::PREV_FRAME, "Left"),
            Self::NextFrame => Spec("next_frame", Playback, text::NEXT_FRAME, "Right"),
            Self::PrevKey => Spec("prev_key", Playback, text::PREV_KEY, "OpenBracket"),
            Self::NextKey => Spec("next_key", Playback, text::NEXT_KEY, "CloseBracket"),
            Self::CutAtPlayhead => Spec("cut_at_playhead", Montage, text::CUT_AT_PLAYHEAD, "C"),
            Self::Select => Spec("select_tool", Montage, text::SELECT, "A"),
            Self::Blade => Spec("blade_tool", Montage, text::BLADE, "B"),
            Self::CutRange => Spec("cut_range", Montage, text::CUT_RANGE, "Shift+Delete"),
            Self::Snap => Spec("snap", Montage, text::SNAP, "N"),
            Self::AddZoom => Spec("add_zoom", Effects, text::ADD_ZOOM, "Z"),
            Self::AddAnnotation => Spec("add_annotation", Effects, text::ADD_ANNOTATION, "T"),
            Self::Keyframe => Spec("keyframe", Effects, text::KEYFRAME, "I"),
            Self::NextAnnotation => Spec("next_annotation", Effects, text::NEXT_ANNOTATION, "Tab"),
            Self::Undo => Spec("undo", Edit, text::UNDO, "Ctrl+Z"),
            Self::Redo => Spec("redo", Edit, text::REDO, "Ctrl+Y"),
            Self::RedoAlt => Spec("redo_alt", Edit, text::REDO_ALT, "Ctrl+Shift+Z"),
            Self::Copy => Spec("copy", Edit, text::COPY, "Ctrl+C"),
            Self::Cut => Spec("cut", Edit, text::CUT, "Ctrl+X"),
            Self::Paste => Spec("paste", Edit, text::PASTE, "Ctrl+V"),
            Self::SelectAll => Spec("select_all", Edit, text::SELECT_ALL, "Ctrl+A"),
            Self::Delete => Spec("delete", Edit, text::DELETE, "Delete"),
            Self::NewProject => Spec("new_project", Project, text::NEW_PROJECT, "Ctrl+N"),
            Self::OpenProject => Spec("open_project", Project, text::OPEN_PROJECT, "Ctrl+O"),
            Self::Save => Spec("save", Project, text::SAVE, "Ctrl+S"),
            Self::SaveAs => Spec("save_as", Project, text::SAVE_AS, "Ctrl+Shift+S"),
            Self::Help => Spec("help", Project, text::HELP, "F1"),
        }
    }

    /// The key of the action in `settings.json`.
    pub fn id(self) -> &'static str {
        self.spec().0
    }

    pub fn group(self) -> Group {
        self.spec().1
    }

    pub fn title(self) -> &'static str {
        self.spec().2
    }

    pub fn default_keys(self) -> &'static str {
        self.spec().3
    }
}

/// Editor shortcuts the user changed, by action id (`null` — removed); the others are the
/// defaults, so a new action gets its default and an unknown id is ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EditorKeys(BTreeMap<String, Option<String>>);

impl EditorKeys {
    /// The shortcut text of `key`, `None` when removed.
    pub fn keys(&self, key: EditorKey) -> Option<String> {
        match self.0.get(key.id()) {
            Some(keys) => keys.clone(),
            None => Some(key.default_keys().to_owned()),
        }
    }

    pub fn shortcut(&self, key: EditorKey) -> Option<Shortcut> {
        self.keys(key).as_deref().and_then(Shortcut::parse)
    }

    /// The caps of the shortcut of `key` for a tooltip: `Ctrl+Z`.
    pub fn label(&self, key: EditorKey) -> Option<String> {
        self.shortcut(key).map(Shortcut::label)
    }

    pub fn set(&mut self, key: EditorKey, keys: Option<String>) {
        if keys.as_deref() == Some(key.default_keys()) {
            self.0.remove(key.id());
        } else {
            self.0.insert(key.id().to_owned(), keys);
        }
    }

    /// The action `pressed` triggers.
    pub fn find(&self, pressed: Shortcut) -> Option<EditorKey> {
        EditorKey::ALL
            .into_iter()
            .find(|key| self.shortcut(*key) == Some(pressed))
    }
}

/// A shortcut the settings reassign: a global hotkey or an editor action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Binding {
    Global(Command),
    Editor(EditorKey),
}

impl Binding {
    pub fn all() -> impl Iterator<Item = Self> {
        let global = Command::ALL.into_iter().map(Self::Global);
        global.chain(EditorKey::ALL.into_iter().map(Self::Editor))
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Global(command) => command.title(),
            Self::Editor(key) => key.title(),
        }
    }

    pub fn keys(self, settings: &AppSettings) -> Option<String> {
        match self {
            Self::Global(command) => command.keys(&settings.hotkeys).clone(),
            Self::Editor(key) => settings.editor_keys.keys(key),
        }
    }

    pub fn set(self, settings: &mut AppSettings, keys: Option<String>) {
        match self {
            Self::Global(command) => *command.keys_mut(&mut settings.hotkeys) = keys,
            Self::Editor(key) => settings.editor_keys.set(key, keys),
        }
    }

    pub fn reset(self, settings: &mut AppSettings) {
        let keys = self.keys(&AppSettings::default());
        self.set(settings, keys);
    }

    /// A global hotkey needs Ctrl or Alt: a bare key would be taken from every program.
    pub fn takes(self, shortcut: Shortcut) -> bool {
        match self {
            Self::Global(_) => shortcut.ctrl || shortcut.alt,
            Self::Editor(_) => true,
        }
    }
}

/// Another binding that already has `shortcut`. Global and editor shortcuts are one space: a
/// global hotkey takes its keys from the editor too.
pub fn taken_by(settings: &AppSettings, shortcut: Shortcut, except: Binding) -> Option<Binding> {
    Binding::all()
        .filter(|b| *b != except)
        .find(|b| b.keys(settings).as_deref().and_then(Shortcut::parse) == Some(shortcut))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_default_parses_and_is_unique() {
        let settings = AppSettings::default();
        let mut ids = HashSet::new();
        for binding in Binding::all() {
            let keys = binding.keys(&settings).unwrap();
            let shortcut = Shortcut::parse(&keys).unwrap();
            assert_eq!(shortcut.text(), keys, "stored as written");
            assert_eq!(taken_by(&settings, shortcut, binding), None, "{keys}");
            if let Binding::Editor(key) = binding {
                assert!(ids.insert(key.id()), "{}", key.id());
            }
        }
    }

    #[test]
    fn a_change_is_stored_and_a_default_is_not() {
        let mut keys = EditorKeys::default();
        keys.set(EditorKey::Blade, Some("Ctrl+B".to_owned()));
        keys.set(EditorKey::Snap, None);
        keys.set(EditorKey::Undo, Some("Ctrl+Z".to_owned()));
        let json = serde_json::to_string(&keys).unwrap();
        assert_eq!(json, r#"{"blade_tool":"Ctrl+B","snap":null}"#);
        let read: EditorKeys =
            serde_json::from_str(r#"{"gone":"F2","blade_tool":"Ctrl+B"}"#).unwrap();
        assert_eq!(read.keys(EditorKey::Blade).as_deref(), Some("Ctrl+B"));
        assert_eq!(read.keys(EditorKey::Select).as_deref(), Some("A"));
        assert_eq!(keys.keys(EditorKey::Snap), None);
        assert_eq!(
            keys.find(Shortcut::parse("Ctrl+B").unwrap()),
            Some(EditorKey::Blade)
        );
        assert_eq!(keys.find(Shortcut::parse("B").unwrap()), None);
    }

    #[test]
    fn a_global_shortcut_conflicts_with_an_editor_one() {
        let mut settings = AppSettings::default();
        let save = Shortcut::parse("Ctrl+S").unwrap();
        let start = Binding::Global(Command::StartStop);
        assert_eq!(
            taken_by(&settings, save, start),
            Some(Binding::Editor(EditorKey::Save))
        );
        settings.hotkeys.start_stop = Some("Ctrl+B".to_owned());
        let blade = Binding::Editor(EditorKey::Blade);
        assert_eq!(
            taken_by(&settings, Shortcut::parse("Ctrl+B").unwrap(), blade),
            Some(start)
        );
        assert!(!start.takes(Shortcut::parse("B").unwrap()));
        assert!(blade.takes(Shortcut::parse("B").unwrap()));
    }
}
