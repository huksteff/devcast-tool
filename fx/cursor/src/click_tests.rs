use dc_core::project::SpeedRange;
use dc_core::{Command, History, Project, Range};
use dc_render::Rgba;
use serde_json::json;

use crate::test_util::{RECORDING, project, s};
use crate::{ClickKind, ClickReset, ClickSet, ClicksSet, Cursor, CursorState};

fn state(p: &Project) -> CursorState {
    p.state::<Cursor>().unwrap()
}

fn clicks(args: serde_json::Value) -> ClicksSet {
    serde_json::from_value(args).unwrap()
}

fn click(args: serde_json::Value) -> ClickSet {
    serde_json::from_value(args).unwrap()
}

fn reset(at: f64) -> ClickReset {
    serde_json::from_value(json!({ "at": at })).unwrap()
}

#[test]
fn clicks_sets_the_style_of_every_click() {
    let mut p = project();
    let diff = clicks(json!({"style": "ripple", "color": "#ef4444ff", "duration": 0.8}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        diff.to_string(),
        "эффект кликов: ripple, #ef4444ff, ×1.00, 100 %, 0.80 с"
    );
    let style = state(&p).settings.click;
    assert_eq!(style.kind, ClickKind::Ripple);
    assert_eq!(style.color, Rgba([0xef, 0x44, 0x44, 0xff]));
    assert!(
        clicks(json!({"style": "ripple"}))
            .apply(&mut p)
            .unwrap()
            .is_empty()
    );
    for bad in [
        json!({"scale": 2.5}),
        json!({"opacity": 1.1}),
        json!({"duration": 0.1}),
        json!({"duration": 1.3}),
    ] {
        let error = clicks(bad.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains("нужно от"), "{bad}: {error}");
    }
    assert!(serde_json::from_value::<ClicksSet>(json!({"color": "#ef4444"})).is_err());
    assert!(serde_json::from_value::<ClicksSet>(json!({"style": "burst"})).is_err());
}

#[test]
fn click_set_changes_one_press_by_its_output_time() {
    let mut p = project();
    p.clips[0].cuts = vec![Range::new(s(0.5), s(1.0)).unwrap()];
    // Output 0.7 s is 1.2 s of the recording.
    let diff = click(json!({"at": 0.7, "style": "off"}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        diff.to_string(),
        "клик 0.700 с: off, #2563ebff, ×1.00, 100 %, 0.60 с"
    );
    click(json!({"at": 0.72, "scale": 1.5}))
        .apply(&mut p)
        .unwrap();
    let changes = state(&p).clicks;
    assert_eq!(changes.len(), 1, "{changes:?}");
    let change = &changes[0];
    assert_eq!((change.media.as_str(), change.at), (RECORDING, s(1.2)));
    assert_eq!(
        (change.kind, change.scale),
        (Some(ClickKind::Off), Some(1.5))
    );
    click(json!({"at": 1.0, "style": "flash"}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(state(&p).clicks.len(), 2);
}

#[test]
fn click_time_counts_speed_and_must_be_in_the_video() {
    let mut p = project();
    p.clips[0].speeds = vec![SpeedRange {
        range: Range::new(s(1.0), s(2.0)).unwrap(),
        factor: 2.0,
    }];
    click(json!({"at": 1.1, "style": "flash"}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(state(&p).clicks[0].at, s(1.2));
    let error = click(json!({"at": 99, "style": "off"}))
        .apply(&mut p)
        .unwrap_err()
        .to_string();
    assert!(error.contains("devcast events clicks"), "{error}");
}

#[test]
fn click_reset_brings_the_common_style_back() {
    let mut p = project();
    click(json!({"at": 1.2, "style": "off"}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        reset(1.23).apply(&mut p).unwrap().to_string(),
        "клик 1.230 с: как все"
    );
    assert!(state(&p).clicks.is_empty());
    assert!(p.effects.is_empty());
    let error = reset(1.2).apply(&mut p).unwrap_err().to_string();
    assert!(error.contains("нет своих настроек"), "{error}");
}

#[test]
fn undo_and_redo_of_click_changes() {
    let mut p = project();
    let mut history = History::default();
    history
        .apply(&mut p, &clicks(json!({"style": "flash"})))
        .unwrap();
    history
        .apply(&mut p, &click(json!({"at": 1.2, "color": "#00ff00ff"})))
        .unwrap();
    let after = p.clone();
    history.undo(&mut p).unwrap();
    assert!(state(&p).clicks.is_empty());
    history.undo(&mut p).unwrap();
    assert_eq!(state(&p), CursorState::default());
    history.redo(&mut p).unwrap();
    history.redo(&mut p).unwrap();
    assert_eq!(p, after);
    let text = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(state(&back), state(&p));
}

#[test]
fn removed_styles_of_older_projects_load_as_ripple() {
    let mut p = project();
    clicks(json!({"style": "flash"})).apply(&mut p).unwrap();
    click(json!({"at": 1.2, "style": "flash"}))
        .apply(&mut p)
        .unwrap();
    let text = serde_json::to_string(&p)
        .unwrap()
        .replacen("\"flash\"", "\"spotlight\"", 1)
        .replacen("\"flash\"", "\"echo\"", 1);
    assert!(text.contains("\"spotlight\"") && text.contains("\"echo\""));
    let mut old: Project = serde_json::from_str(&text).unwrap();
    let loaded = state(&old);
    assert_eq!(loaded.settings.click.kind, ClickKind::Ripple);
    assert_eq!(loaded.clicks[0].kind, Some(ClickKind::Ripple));
    // The next edit of the effect writes its state back with the new name.
    clicks(json!({"opacity": 0.5})).apply(&mut old).unwrap();
    let saved = serde_json::to_string(&old).unwrap();
    assert!(
        !saved.contains("spotlight") && !saved.contains("echo"),
        "{saved}"
    );
}
