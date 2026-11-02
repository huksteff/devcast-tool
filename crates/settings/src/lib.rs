//! The app settings `%APPDATA%\DevCast\settings.json`: their model and defaults, the file, the
//! table of shortcuts and the edits of `devcast config` and `devcast hotkeys`. Shared by the app
//! (its one `Store` for all windows) and `devcast` (the file when the app is not running).

pub mod edit;
pub mod keymap;
pub mod model;
pub mod placement;
pub mod shortcut;
pub mod store;

pub use edit::{Applied, Edit, EditError, Key, Setting, apply};
pub use keymap::{Binding, Command, EditorKey, EditorKeys, Group, taken_by};
pub use model::{
    AppSettings, Countdown, EditorLayout, FILE, Hotkeys, InspectorGroups, Language, Listen,
    SpeechModel, Updates, WindowPlace,
};
pub use placement::HudPlacement;
pub use shortcut::Shortcut;
pub use store::{Store, StoreError, file};
