use dc_core::{Range, Rect, TimeMap};
use dc_render::Sprite;
use serde_json::json;

use super::*;
use crate::state::Smoothing;
use crate::test_util::{HAND, HD, o, project, s, tracks};

fn settings(smoothing: Smoothing) -> CursorSettings {
    CursorSettings {
        smoothing,
        ..CursorSettings::default()
    }
}

fn scene_at(project: &Project, settings: CursorSettings, secs: f64) -> Scene {
    let prepared = Prepared::new(project, settings, tracks());
    let mut scene = Scene::new(HD);
    evaluate(
        &prepared,
        project,
        &TimeMap::build(project),
        o(secs),
        &mut scene,
    );
    scene
}

fn cursor(scene: &Scene) -> Option<&Sprite> {
    scene.painted().find_map(|(band, layer)| match layer {
        Layer::Sprite(sprite) if band == Band::Cursor => Some(sprite),
        _ => None,
    })
}

/// Frame px of the hotspot of a 32 px picture with hotspot (`hx`, `hy`).
fn tip(sprite: &Sprite, hx: f32, hy: f32) -> [f32; 2] {
    let r = sprite.rect;
    [r.x + hx / 32.0 * r.w, r.y + hy / 32.0 * r.h]
}

#[test]
fn cursor_through_the_recording_and_the_imported_clip() {
    let mut p = project();
    // Cut 1.5–2 s and 2× on 3–4 s: output 0–1.5 is 0–1.5, 1.5–2.5 is 2–3, 2.5–3 is 3–4.
    p.clips[0].cuts = vec![Range::new(s(1.5), s(2.0)).unwrap()];
    p.clips[0].speeds = vec![dc_core::project::SpeedRange {
        range: Range::new(s(3.0), s(4.0)).unwrap(),
        factor: 2.0,
    }];
    let moments = [
        0.0, 0.5, 0.55, 0.6, 0.75, 1.0, 1.1, 1.49, 1.5, 2.0, 2.4, 2.5, 2.7, 2.9, 3.0, 4.0,
    ];
    let scenes: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "cursor": cursor(&scene_at(&p, settings(Smoothing::Focused), t)) }))
        .collect();
    insta::assert_json_snapshot!(scenes);
}

#[test]
fn exact_motion_puts_the_hotspot_on_the_recorded_point() {
    let p = project();
    let scene = scene_at(&p, settings(Smoothing::Exact), 0.75);
    let sprite = cursor(&scene).unwrap();
    // Halfway through the sweep, as recorded (the event of 0.75 s, step 1/62).
    let q = 31.0 / 62.0;
    let expected = [(0.2 + 0.6 * q) * 1920.0, (0.3 + 0.4 * q) * 1080.0];
    let [x, y] = tip(sprite, 0.0, 0.0);
    assert!(
        (f64::from(x) - expected[0]).abs() < 0.01,
        "{x} {expected:?}"
    );
    assert!(
        (f64::from(y) - expected[1]).abs() < 0.01,
        "{y} {expected:?}"
    );
}

#[test]
fn smoothing_lags_behind_and_catches_up() {
    let p = project();
    let exact = tip(
        cursor(&scene_at(&p, settings(Smoothing::Exact), 0.75)).unwrap(),
        0.0,
        0.0,
    );
    for smoothing in [Smoothing::Focused, Smoothing::Smooth] {
        let lag = tip(
            cursor(&scene_at(&p, settings(smoothing), 0.75)).unwrap(),
            0.0,
            0.0,
        );
        assert!(lag[0] < exact[0] - 10.0, "{smoothing:?}: {lag:?} {exact:?}");
        let rest = tip(
            cursor(&scene_at(&p, settings(smoothing), 2.45)).unwrap(),
            0.0,
            0.0,
        );
        assert!(
            (rest[0] - 0.8 * 1920.0).abs() < 1.5,
            "{smoothing:?}: {rest:?}"
        );
    }
}

#[test]
fn size_scales_with_the_clip_width() {
    let p = project();
    let sprite = cursor(&scene_at(&p, CursorSettings::default(), 0.2))
        .unwrap()
        .clone();
    // Size 2.5 at 1920 px: the arrow 70 px, the picture 70 · 32 / 19.0625.
    assert!((sprite.rect.h - 117.508).abs() < 0.01, "{:?}", sprite.rect);
    assert_eq!(sprite.rect.w, sprite.rect.h);
    let big = CursorSettings {
        size: 10.0,
        ..CursorSettings::default()
    };
    let large = cursor(&scene_at(&p, big, 0.2)).unwrap().clone();
    assert!((large.rect.h - 4.0 * sprite.rect.h).abs() < 0.01);
    assert_eq!(tip(&large, 0.0, 0.0), tip(&sprite, 0.0, 0.0));
}

#[test]
fn crop_moves_and_hides_the_cursor() {
    let mut p = project();
    // The right half: (0.2, 0.3) is outside, the end of the sweep (0.8, 0.7) inside.
    p.clips[0].crop = Rect::new(0.5, 0.0, 0.5, 1.0).unwrap();
    let exact = settings(Smoothing::Exact);
    assert_eq!(cursor(&scene_at(&p, exact.clone(), 0.2)), None);
    let scene = scene_at(&p, exact, 1.3);
    let sprite = cursor(&scene).unwrap();
    // The 960 × 1080 crop is fitted into 1920 × 1080 with fields: 480 px on each side.
    let [x, y] = tip(sprite, 0.0, 0.0);
    assert!((x - (480.0 + 0.6 * 960.0)).abs() < 0.01, "{x}");
    assert!((y - 0.7 * 1080.0).abs() < 0.01, "{y}");
    // Size follows the shown width of the clip.
    assert!((sprite.rect.h - 117.508 / 2.0).abs() < 0.01);
}

#[test]
fn hidden_cursor_and_hidden_setting_draw_nothing() {
    let p = project();
    assert_eq!(cursor(&scene_at(&p, CursorSettings::default(), 3.4)), None);
    let shown = scene_at(&p, CursorSettings::default(), 3.55);
    assert_eq!(cursor(&shown).unwrap().image.path, HAND);
    let hidden = CursorSettings {
        show: false,
        ..CursorSettings::default()
    };
    assert_eq!(scene_at(&p, hidden, 1.0), Scene::new(HD));
}

#[test]
fn imported_clip_and_frames_outside_the_video_have_no_cursor() {
    let p = project();
    assert_eq!(scene_at(&p, CursorSettings::default(), 4.5), Scene::new(HD));
    assert_eq!(scene_at(&p, CursorSettings::default(), 7.0), Scene::new(HD));
    let empty = Prepared::new(&p, CursorSettings::default(), CursorTracks::new());
    let mut scene = Scene::new(HD);
    evaluate(&empty, &p, &TimeMap::build(&p), o(1.0), &mut scene);
    assert_eq!(scene, Scene::new(HD));
}
