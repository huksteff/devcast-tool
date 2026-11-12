use dc_core::project::SpeedRange;
use dc_core::{CursorTracks, Project, Range, Rect, TimeMap};
use dc_render::{Band, Geometry, Layer, Scene, Shape};
use serde_json::json;

use crate::click_state::{ClickChange, ClickKind, ClickStyle};
use crate::test_util::{HD, RECORDING, lively, o, point, project, s, track};
use crate::{CursorSettings, CursorState, Prepared, evaluate};

/// Clicks at 1.2 s (0.8, 0.7), then 1.5 and 1.6 s at (0.25, 0.25).
fn tracks() -> CursorTracks {
    let mut track = track();
    track.clicks = vec![
        point(1.2, 0.8, 0.7),
        point(1.5, 0.25, 0.25),
        point(1.6, 0.25, 0.25),
    ];
    CursorTracks::from([(RECORDING.to_owned(), track)])
}

fn style(kind: ClickKind) -> CursorState {
    CursorState {
        settings: CursorSettings {
            click: ClickStyle {
                kind,
                ..ClickStyle::default()
            },
            ..lively()
        },
        clicks: Vec::new(),
    }
}

fn effects_at(project: &Project, state: &CursorState, secs: f64) -> Vec<Shape> {
    let prepared = Prepared::new(project, state.clone(), &tracks());
    let mut scene = Scene::new(HD);
    evaluate(
        &prepared,
        project,
        &TimeMap::build(project),
        o(secs),
        &mut scene,
    );
    let layers: Vec<_> = scene
        .painted()
        .filter_map(|(band, layer)| match layer {
            Layer::Shapes(shapes) => Some((band, shapes.clone())),
            _ => None,
        })
        .collect();
    assert!(layers.len() <= 1 && layers.iter().all(|(band, _)| *band == Band::Cursor));
    layers.into_iter().flat_map(|(_, shapes)| shapes).collect()
}

/// Centre and radius of a circle.
fn circle(shape: &Shape) -> [f32; 3] {
    let Geometry::Ellipse { rect } = shape.geometry else {
        panic!("not a circle: {shape:?}");
    };
    [rect.x + rect.w / 2.0, rect.y + rect.h / 2.0, rect.w / 2.0]
}

#[test]
fn ripple_through_its_life_and_overlapping_clicks() {
    let p = project();
    let state = style(ClickKind::Ripple);
    let moments = [
        1.2, 1.374, 1.376, 1.45, 1.6, 1.8, 1.97, 1.98, 1.7, 1.9, 2.3, 2.5,
    ];
    let frames: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "effects": effects_at(&p, &state, t) }))
        .collect();
    insta::assert_json_snapshot!(frames);
}

#[test]
fn every_style_at_one_moment() {
    let p = project();
    let kinds = [ClickKind::Ripple, ClickKind::Flash];
    let looks: Vec<_> = kinds
        .iter()
        .map(|&kind| json!({ "kind": kind, "effects": effects_at(&p, &style(kind), 1.55) }))
        .collect();
    insta::assert_json_snapshot!(looks);
}

#[test]
fn effect_starts_half_a_bounce_after_the_press_and_lasts_its_duration() {
    let p = project();
    let state = style(ClickKind::Ripple);
    assert!(effects_at(&p, &state, 1.374).is_empty());
    let first = effects_at(&p, &state, 1.376);
    assert_eq!(first.len(), 1);
    for shape in &first {
        let [x, y, _] = circle(shape);
        assert!((x - 1536.0).abs() < 0.01 && (y - 756.0).abs() < 0.01);
    }
    assert!(!effects_at(&p, &state, 1.97).is_empty());
    let later = effects_at(&p, &state, 1.98);
    assert!(later.iter().all(|e| (circle(e)[0] - 480.0).abs() < 0.01));
    // 1.75 s: the click of 1.2 s and the one of 1.5 s both run, the older one under.
    let both = effects_at(&p, &state, 1.75);
    assert_eq!(both.len(), 2);
    assert!((circle(&both[0])[0] - 1536.0).abs() < 0.01);
    assert!((circle(&both[1])[0] - 480.0).abs() < 0.01);
    assert!(effects_at(&p, &CursorState::default(), 1.5).is_empty());
}

#[test]
fn effects_stay_with_the_cursor_hidden_and_leave_with_the_crop() {
    let mut p = project();
    let mut state = style(ClickKind::Ripple);
    state.settings.show = false;
    assert_eq!(effects_at(&p, &state, 1.5).len(), 1);
    // The right half: the click of 1.2 s is inside, those at (0.25, 0.25) are not.
    p.clips[0].crop = Rect::new(0.5, 0.0, 0.5, 1.0).unwrap();
    assert_eq!(effects_at(&p, &state, 1.5).len(), 1);
    assert!(effects_at(&p, &state, 2.1).is_empty());
}

#[test]
fn effects_run_at_normal_speed_on_a_sped_up_part() {
    let mut p = project();
    p.clips[0].speeds = vec![SpeedRange {
        range: Range::new(s(1.0), s(2.0)).unwrap(),
        factor: 2.0,
    }];
    let state = style(ClickKind::Ripple);
    // The press plays at output 1.1 s: 1.275–1.875 s.
    assert!(effects_at(&p, &state, 1.27).is_empty());
    assert!(!effects_at(&p, &state, 1.28).is_empty());
    let late = effects_at(&p, &state, 1.87);
    assert!(late.iter().any(|e| (circle(e)[0] - 1536.0).abs() < 0.01));
}

#[test]
fn ripple_grows_to_twice_the_arrow_by_its_scale() {
    let p = project();
    let mut state = style(ClickKind::Ripple);
    let end = circle(&effects_at(&p, &state, 1.97)[0])[2];
    // Arrow 70 px at size 2.5 in 1920 px: 1.95 × 70 at the end.
    assert!((end - 136.5).abs() < 1.0, "{end}");
    state.settings.click.scale = 2.0;
    let big = circle(&effects_at(&p, &state, 1.97)[0])[2];
    assert!((big - 2.0 * end).abs() < 2.0, "{big}");
}

#[test]
fn a_changed_click_has_its_own_look() {
    let p = project();
    let mut state = style(ClickKind::Ripple);
    let change = |at: f64, kind| ClickChange {
        media: RECORDING.to_owned(),
        at: s(at),
        kind: Some(kind),
        color: None,
        scale: None,
        opacity: None,
        duration: None,
    };
    // Off for the press at 1.2 s, typed 40 ms off; a flash (a disk and 18 particles) for the one
    // at 1.5 s.
    state.clicks = vec![change(1.24, ClickKind::Off), change(1.5, ClickKind::Flash)];
    assert!(effects_at(&p, &state, 1.45).is_empty());
    assert_eq!(effects_at(&p, &state, 1.75).len(), 19);
    assert_eq!(effects_at(&p, &state, 1.85).len(), 20);
    // 60 ms is another press.
    state.clicks = vec![change(1.26, ClickKind::Off)];
    assert_eq!(effects_at(&p, &state, 1.45).len(), 1);
}
