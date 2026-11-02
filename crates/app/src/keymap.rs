//! The table of shortcuts (`dc_settings::keymap`: ids, groups, defaults, conflicts) with the
//! names the windows show. The editor dispatches by it (`editor::keys`), the help and the
//! tooltips read it, the settings reassign it, `devcast hotkeys` changes the same table.

use dc_ui_kit::strings::{hotkeys, keys as text};

pub use dc_settings::keymap::{Binding, Command, EditorKey, EditorKeys, Group, taken_by};

use crate::shortcut::Caps;

/// The name a window shows.
pub trait Title {
    fn title(self) -> &'static str;
}

impl Title for Group {
    fn title(self) -> &'static str {
        match self {
            Self::Playback => text::PLAYBACK,
            Self::Montage => text::MONTAGE,
            Self::Effects => text::EFFECTS,
            Self::Edit => text::EDIT,
            Self::Project => text::PROJECT,
        }
    }
}

impl Title for Command {
    fn title(self) -> &'static str {
        match self {
            Self::StartStop => hotkeys::START_STOP,
            Self::Pause => hotkeys::PAUSE,
            Self::Cancel => hotkeys::CANCEL,
        }
    }
}

impl Title for EditorKey {
    fn title(self) -> &'static str {
        match self {
            Self::PlayPause => text::PLAY_PAUSE,
            Self::PrevFrame => text::PREV_FRAME,
            Self::NextFrame => text::NEXT_FRAME,
            Self::PrevKey => text::PREV_KEY,
            Self::NextKey => text::NEXT_KEY,
            Self::CutAtPlayhead => text::CUT_AT_PLAYHEAD,
            Self::Select => text::SELECT,
            Self::Blade => text::BLADE,
            Self::CutRange => text::CUT_RANGE,
            Self::RangeSpeed => text::RANGE_SPEED,
            Self::Snap => text::SNAP,
            Self::AddZoom => text::ADD_ZOOM,
            Self::AddAnnotation => text::ADD_ANNOTATION,
            Self::Keyframe => text::KEYFRAME,
            Self::NextAnnotation => text::NEXT_ANNOTATION,
            Self::Undo => text::UNDO,
            Self::Redo => text::REDO,
            Self::RedoAlt => text::REDO_ALT,
            Self::Copy => text::COPY,
            Self::Cut => text::CUT,
            Self::Paste => text::PASTE,
            Self::SelectAll => text::SELECT_ALL,
            Self::Delete => text::DELETE,
            Self::NewProject => text::NEW_PROJECT,
            Self::OpenProject => text::OPEN_PROJECT,
            Self::Save => text::SAVE,
            Self::SaveAs => text::SAVE_AS,
            Self::Help => text::HELP,
        }
    }
}

impl Title for Binding {
    fn title(self) -> &'static str {
        match self {
            Self::Global(command) => command.title(),
            Self::Editor(key) => key.title(),
        }
    }
}

/// The caps of a shortcut for a tooltip.
pub trait Label {
    /// The caps of the shortcut of `key`: `Ctrl+Z`; `None` when it is cleared.
    fn label(&self, key: EditorKey) -> Option<String>;
}

impl Label for EditorKeys {
    fn label(&self, key: EditorKey) -> Option<String> {
        self.shortcut(key).map(Caps::label)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_binding_has_its_own_name() {
        let mut titles = HashSet::new();
        for binding in Binding::all() {
            assert!(titles.insert(binding.title()), "{}", binding.title());
        }
    }
}
