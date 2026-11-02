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
        assert!(ids.insert(binding.id()), "{}", binding.id());
        assert_eq!(Binding::find(binding.id()), Some(binding));
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
    let read: EditorKeys = serde_json::from_str(r#"{"gone":"F2","blade_tool":"Ctrl+B"}"#).unwrap();
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
