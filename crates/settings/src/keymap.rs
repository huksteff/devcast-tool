//! The table of shortcuts (roadmap 7.3): the global hotkeys of recording and every editor action,
//! each with its id (the key in `settings.json` and in `devcast hotkeys`), group and default
//! shortcut by physical key. The names people read are the app's (dc-ui-kit). Defaults follow
//! the mockup and the owner's decisions (A «Выбор», B «Лезвие», T annotation, Shift+Delete);
//! all of them can be changed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{AppSettings, Hotkeys};
use crate::shortcut::Shortcut;

/// What a global hotkey does: it works from any program and with the HUD hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    StartStop,
    Pause,
    Cancel,
}

impl Command {
    pub const ALL: [Self; 3] = [Self::StartStop, Self::Pause, Self::Cancel];

    /// The id, as the field of [`Hotkeys`].
    pub fn id(self) -> &'static str {
        match self {
            Self::StartStop => "start_stop",
            Self::Pause => "pause",
            Self::Cancel => "cancel",
        }
    }

    pub fn keys(self, hotkeys: &Hotkeys) -> &Option<String> {
        match self {
            Self::StartStop => &hotkeys.start_stop,
            Self::Pause => &hotkeys.pause,
            Self::Cancel => &hotkeys.cancel,
        }
    }

    pub fn keys_mut(self, hotkeys: &mut Hotkeys) -> &mut Option<String> {
        match self {
            Self::StartStop => &mut hotkeys.start_stop,
            Self::Pause => &mut hotkeys.pause,
            Self::Cancel => &mut hotkeys.cancel,
        }
    }
}

/// Section of the editor's help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Playback,
    Montage,
    Effects,
    Edit,
    Project,
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
    RangeSpeed,
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

impl EditorKey {
    pub const ALL: [Self; 28] = [
        Self::PlayPause,
        Self::PrevFrame,
        Self::NextFrame,
        Self::PrevKey,
        Self::NextKey,
        Self::CutAtPlayhead,
        Self::Select,
        Self::Blade,
        Self::CutRange,
        Self::RangeSpeed,
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

    /// Id, group and default shortcut.
    fn spec(self) -> (&'static str, Group, &'static str) {
        use Group::{Edit, Effects, Montage, Playback, Project};
        match self {
            Self::PlayPause => ("play_pause", Playback, "Space"),
            Self::PrevFrame => ("prev_frame", Playback, "Left"),
            Self::NextFrame => ("next_frame", Playback, "Right"),
            Self::PrevKey => ("prev_key", Playback, "OpenBracket"),
            Self::NextKey => ("next_key", Playback, "CloseBracket"),
            Self::CutAtPlayhead => ("cut_at_playhead", Montage, "C"),
            Self::Select => ("select_tool", Montage, "A"),
            Self::Blade => ("blade_tool", Montage, "B"),
            Self::CutRange => ("cut_range", Montage, "Shift+Delete"),
            Self::RangeSpeed => ("range_speed", Montage, "R"),
            Self::Snap => ("snap", Montage, "N"),
            Self::AddZoom => ("add_zoom", Effects, "Z"),
            Self::AddAnnotation => ("add_annotation", Effects, "T"),
            Self::Keyframe => ("keyframe", Effects, "I"),
            Self::NextAnnotation => ("next_annotation", Effects, "Tab"),
            Self::Undo => ("undo", Edit, "Ctrl+Z"),
            Self::Redo => ("redo", Edit, "Ctrl+Y"),
            Self::RedoAlt => ("redo_alt", Edit, "Ctrl+Shift+Z"),
            Self::Copy => ("copy", Edit, "Ctrl+C"),
            Self::Cut => ("cut", Edit, "Ctrl+X"),
            Self::Paste => ("paste", Edit, "Ctrl+V"),
            Self::SelectAll => ("select_all", Edit, "Ctrl+A"),
            Self::Delete => ("delete", Edit, "Delete"),
            Self::NewProject => ("new_project", Project, "Ctrl+N"),
            Self::OpenProject => ("open_project", Project, "Ctrl+O"),
            Self::Save => ("save", Project, "Ctrl+S"),
            Self::SaveAs => ("save_as", Project, "Ctrl+Shift+S"),
            Self::Help => ("help", Project, "F1"),
        }
    }

    /// The key of the action in `settings.json`.
    pub fn id(self) -> &'static str {
        self.spec().0
    }

    pub fn group(self) -> Group {
        self.spec().1
    }

    pub fn default_keys(self) -> &'static str {
        self.spec().2
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

/// A shortcut the settings reassign: a global hotkey or an editor action. Known by its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Binding {
    Global(Command),
    Editor(EditorKey),
}

impl Binding {
    pub fn all() -> impl Iterator<Item = Self> {
        let global = Command::ALL.into_iter().map(Self::Global);
        global.chain(EditorKey::ALL.into_iter().map(Self::Editor))
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Global(command) => command.id(),
            Self::Editor(key) => key.id(),
        }
    }

    pub fn find(id: &str) -> Option<Self> {
        Self::all().find(|b| b.id() == id)
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

    pub fn default_keys(self) -> Option<String> {
        self.keys(&AppSettings::default())
    }

    pub fn reset(self, settings: &mut AppSettings) {
        self.set(settings, self.default_keys());
    }

    /// A global hotkey needs Ctrl or Alt: a bare key would be taken from every program.
    pub fn takes(self, shortcut: Shortcut) -> bool {
        match self {
            Self::Global(_) => shortcut.ctrl || shortcut.alt,
            Self::Editor(_) => true,
        }
    }
}

impl TryFrom<String> for Binding {
    type Error = String;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::find(&id).ok_or_else(|| format!("нет сочетания с id «{id}»"))
    }
}

impl From<Binding> for String {
    fn from(binding: Binding) -> Self {
        binding.id().to_owned()
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
#[path = "keymap_tests.rs"]
mod tests;
