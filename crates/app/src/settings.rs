//! The app settings, one file `%APPDATA%\DevCast\settings.json` (dc-settings): the HUD and the
//! settings window read and change the same [`Store`] in `Shared`, so a change in one is seen in
//! the other at once (owner's decision on the mockups), and so is a change `devcast config` or
//! `devcast hotkeys` sends (`dc_ops::live::Request::Settings`). Each change is written
//! atomically right away.

pub use dc_settings::{
    AppSettings, Countdown, EditorLayout, FILE, Hotkeys, InspectorGroups, Language, Listen,
    SpeechModel, Store, WindowPlace,
};
