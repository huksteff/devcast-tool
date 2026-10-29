use dc_core::{Command, History, Project};
use serde_json::json;

use crate::test_util::project;
use crate::{Keys, KeysSet, KeysSettings, KeysState, Position};

fn set(args: serde_json::Value) -> KeysSet {
    serde_json::from_value(args).unwrap()
}

fn settings(p: &Project) -> KeysSettings {
    p.state::<Keys>().unwrap().settings
}

#[test]
fn off_by_default_at_the_bottom() {
    let s = KeysSettings::default();
    assert!(!s.show);
    assert_eq!((s.position, s.size), (Position::Bottom, 1.0));
    assert_eq!(s.duration.as_secs_f64(), 1.5);
    assert_eq!(project().state::<Keys>().unwrap(), KeysState::default());
}

#[test]
fn set_changes_only_given_fields_and_says_what() {
    let mut p = project();
    let diff = set(json!({"show": true, "position": "top_right", "size": 1.5}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        diff.to_string(),
        "клавиши: показаны, позиция bottom→top_right, размер 1.00→1.50"
    );
    let diff = set(json!({"color": "#ffd60aff", "background": "#00000080", "duration": 3}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        diff.to_string(),
        "клавиши: цвет #ffffffff→#ffd60aff, плашка #111827d9→#00000080, время показа (с) 1.50→3.00"
    );
    let s = settings(&p);
    assert!(s.show && s.position == Position::TopRight && s.size == 1.5);
    let diff = set(json!({"show": false})).apply(&mut p).unwrap();
    assert_eq!(diff.to_string(), "клавиши: скрыты");
}

#[test]
fn same_values_change_nothing() {
    let mut p = project();
    let diff = set(json!({"show": false, "size": 1, "position": "bottom"}))
        .apply(&mut p)
        .unwrap();
    assert!(diff.is_empty());
    assert!(p.effects.is_empty());
}

#[test]
fn values_outside_the_ranges_are_refused() {
    let mut p = project();
    for bad in [
        json!({"size": 0.49}),
        json!({"size": 3.01}),
        json!({"duration": 0.29}),
        json!({"duration": 5.01}),
    ] {
        let error = set(bad.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains("нужно от"), "{bad}: {error}");
    }
    for good in [
        json!({"size": 0.5}),
        json!({"size": 3}),
        json!({"duration": 0.3}),
    ] {
        set(good).apply(&mut p).unwrap();
    }
    assert!(serde_json::from_value::<KeysSet>(json!({"position": "middle"})).is_err());
    assert!(serde_json::from_value::<KeysSet>(json!({"color": "white"})).is_err());
    assert!(serde_json::from_value::<KeysSet>(json!({"font": "Inter"})).is_err());
}

#[test]
fn undo_and_redo_return_the_same_settings() {
    let mut p = project();
    let mut history = History::default();
    history
        .apply(&mut p, &set(json!({"show": true, "position": "top"})))
        .unwrap();
    let after = p.clone();
    history.undo(&mut p).unwrap();
    assert_eq!(settings(&p), KeysSettings::default());
    history.redo(&mut p).unwrap();
    assert_eq!(p, after);
}

#[test]
fn settings_round_trip_through_the_project_file() {
    let mut p = project();
    set(json!({"show": true, "position": "bottom_left", "duration": 2.5}))
        .apply(&mut p)
        .unwrap();
    let text = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(settings(&back), settings(&p));
    p.effects
        .insert("keys".to_owned(), json!({"settings": {"size": "big"}}));
    assert!(p.state::<Keys>().is_err());
}
