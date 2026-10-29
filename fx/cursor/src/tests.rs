use dc_core::{Command, History, Project};
use serde_json::json;

use crate::test_util::project;
use crate::{Cursor, CursorSet, CursorSettings, CursorState, Smoothing};

fn set(args: serde_json::Value) -> CursorSet {
    serde_json::from_value(args).unwrap()
}

fn settings(p: &Project) -> CursorSettings {
    p.state::<Cursor>().unwrap().settings
}

#[test]
fn defaults_follow_v1_focused() {
    let s = CursorSettings::default();
    assert!(s.show);
    assert_eq!(s.size, 2.5);
    assert_eq!(s.smoothing, Smoothing::Focused);
    assert_eq!(project().state::<Cursor>().unwrap(), CursorState::default());
}

#[test]
fn set_changes_only_given_fields_and_says_what() {
    let mut p = project();
    let diff = set(json!({"size": 3, "smoothing": "exact"}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        diff.to_string(),
        "курсор: размер 2.50→3.00, сглаживание focused→exact"
    );
    let s = settings(&p);
    assert_eq!((s.size, s.smoothing, s.show), (3.0, Smoothing::Exact, true));
    let diff = set(json!({"show": false})).apply(&mut p).unwrap();
    assert_eq!(diff.to_string(), "курсор: скрыт");
}

#[test]
fn same_values_change_nothing() {
    let mut p = project();
    let diff = set(json!({"size": 2.5, "show": true}))
        .apply(&mut p)
        .unwrap();
    assert!(diff.is_empty());
    assert!(p.effects.is_empty());
}

#[test]
fn size_outside_the_range_is_refused() {
    let mut p = project();
    for size in [0.49, 10.01, -1.0] {
        let error = set(json!({ "size": size })).apply(&mut p).unwrap_err();
        assert!(error.to_string().contains("от 0.5 до 10"), "{error}");
    }
    for size in [0.5, 10.0] {
        set(json!({ "size": size })).apply(&mut p).unwrap();
    }
    assert!(serde_json::from_value::<CursorSet>(json!({"smoothing": "soft"})).is_err());
    assert!(serde_json::from_value::<CursorSet>(json!({"colour": 1})).is_err());
}

#[test]
fn undo_and_redo_return_the_same_settings() {
    let mut p = project();
    let mut history = History::default();
    history
        .apply(&mut p, &set(json!({"size": 4, "smoothing": "smooth"})))
        .unwrap();
    let after = p.clone();
    history.undo(&mut p).unwrap();
    assert_eq!(settings(&p), CursorSettings::default());
    history.redo(&mut p).unwrap();
    assert_eq!(p, after);
}

#[test]
fn settings_round_trip_through_the_project_file() {
    let mut p = project();
    set(json!({"size": 1.25, "smoothing": "smooth", "show": false}))
        .apply(&mut p)
        .unwrap();
    let text = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(settings(&back), settings(&p));
    let broken = json!({"settings": {"size": "big"}});
    p.effects.insert("cursor".to_owned(), broken);
    assert!(p.state::<Cursor>().is_err());
}
