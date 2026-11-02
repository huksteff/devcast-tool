use dc_capture::{Fps, Source};

use super::*;
use crate::keymap::{Command, EditorKey};

fn shortcut(text: &str) -> Option<Shortcut> {
    Some(Shortcut::parse(text).unwrap())
}

#[test]
fn set_and_reset_name_what_changed() {
    let start = AppSettings::default();
    let edit = Edit::Set(vec![
        Setting::Fps(Fps::Fps120),
        Setting::Mic(Some("Микрофон (Rode NT-USB)".to_owned())),
        Setting::Source(Source::Monitor(1)),
    ]);
    let applied = apply(&start, edit).unwrap();
    assert_eq!(
        applied.keys,
        [Key::Fps, Key::Mic],
        "the source was already screen:1"
    );
    assert!(applied.bindings.is_empty());
    assert_eq!(applied.settings.recording.fps, Fps::Fps120);
    let back = apply(&applied.settings, Edit::Reset(vec![Key::Fps])).unwrap();
    assert_eq!(back.keys, [Key::Fps]);
    assert_eq!(
        back.settings.recording.microphone.as_deref(),
        Some("Микрофон (Rode NT-USB)")
    );
    let all = apply(&applied.settings, Edit::Reset(Vec::new())).unwrap();
    assert_eq!(all.settings, start);
}

#[test]
fn reset_keeps_what_config_does_not_own() {
    let mut start = AppSettings {
        recent_projects: vec!["C:/Мои видео/Урок 1.devcast".into()],
        ..AppSettings::default()
    };
    start.hotkeys.pause = None;
    let applied = apply(&start, Edit::Reset(Vec::new())).unwrap();
    assert_eq!(applied.settings, start);
    assert!(applied.keys.is_empty());
}

#[test]
fn a_taken_shortcut_is_refused_or_swapped() {
    let start = AppSettings::default();
    let blade = Binding::Editor(EditorKey::Blade);
    let save = Binding::Editor(EditorKey::Save);
    let taken = Edit::Bind {
        binding: blade,
        keys: shortcut("Ctrl+S"),
        swap: false,
    };
    assert_eq!(
        apply(&start, taken),
        Err(EditError::Taken {
            keys: "Ctrl+S".to_owned(),
            owner: "save".to_owned()
        })
    );
    let swapped = Edit::Bind {
        binding: blade,
        keys: shortcut("Ctrl+S"),
        swap: true,
    };
    let applied = apply(&start, swapped).unwrap();
    assert_eq!(applied.bindings, [blade, save]);
    assert_eq!(blade.keys(&applied.settings).as_deref(), Some("Ctrl+S"));
    assert_eq!(save.keys(&applied.settings).as_deref(), Some("B"));
}

#[test]
fn a_global_shortcut_needs_ctrl_or_alt_also_in_a_swap() {
    let start = AppSettings::default();
    let start_stop = Binding::Global(Command::StartStop);
    let bare = Edit::Bind {
        binding: start_stop,
        keys: shortcut("F9"),
        swap: false,
    };
    let refused = Err(EditError::NeedsModifier {
        binding: "start_stop".to_owned(),
    });
    assert_eq!(apply(&start, bare), refused);
    let blade = Edit::Bind {
        binding: Binding::Editor(EditorKey::Blade),
        keys: shortcut("Ctrl+Shift+R"),
        swap: true,
    };
    assert_eq!(
        apply(&start, blade),
        refused,
        "start_stop would get a bare B"
    );
}

#[test]
fn cleared_and_reset_shortcuts() {
    let start = AppSettings::default();
    let pause = Binding::Global(Command::Pause);
    let cleared = Edit::Bind {
        binding: pause,
        keys: None,
        swap: false,
    };
    let applied = apply(&start, cleared).unwrap();
    assert_eq!(applied.settings.hotkeys.pause, None);
    let back = apply(&applied.settings, Edit::ResetKeys(Vec::new())).unwrap();
    assert_eq!(back.bindings, [pause]);
    assert_eq!(back.settings, start);
}

#[test]
fn edits_travel_as_json() {
    let edit = Edit::Bind {
        binding: Binding::Global(Command::StartStop),
        keys: shortcut("ctrl+alt+r"),
        swap: true,
    };
    let json = serde_json::to_string(&edit).unwrap();
    assert_eq!(
        json,
        r#"{"bind":{"binding":"start_stop","keys":"Ctrl+Alt+R","swap":true}}"#
    );
    assert_eq!(serde_json::from_str::<Edit>(&json).unwrap(), edit);
    let set = Edit::Set(vec![Setting::Source(Source::Monitor(2))]);
    let json = serde_json::to_string(&set).unwrap();
    assert_eq!(serde_json::from_str::<Edit>(&json).unwrap(), set);
}
