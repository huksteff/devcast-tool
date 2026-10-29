use dc_core::project::SpeedRange;
use dc_core::{Range, TimeDelta};
use dc_render::Sprite;
use serde_json::json;

use super::*;
use crate::state::Smoothing;
use crate::test_util::{HD, o, project, s, tracks};

/// Exact motion, everything of 5.22 off; tests turn one thing on.
fn exact() -> CursorSettings {
    CursorSettings {
        smoothing: Smoothing::Exact,
        blur: 0.0,
        bounce: 0.0,
        sway: 0.0,
        ..CursorSettings::default()
    }
}

fn sprite_at(project: &Project, settings: CursorSettings, secs: f64) -> Option<Sprite> {
    let prepared = Prepared::new(project, settings, tracks());
    let mut scene = Scene::new(HD);
    evaluate(
        &prepared,
        project,
        &TimeMap::build(project),
        o(secs),
        &mut scene,
    );
    scene.painted().find_map(|(_, layer)| match layer {
        Layer::Sprite(sprite) => Some(sprite.clone()),
        _ => None,
    })
}

/// Frame px of the hotspot (0, 0) of a picture turned around its centre.
fn arrow_tip(sprite: &Sprite) -> [f64; 2] {
    let r = sprite.rect;
    let (w, h) = (f64::from(r.w), f64::from(r.h));
    let centre = [f64::from(r.x) + w / 2.0, f64::from(r.y) + h / 2.0];
    let (sin, cos) = f64::from(sprite.rotation).to_radians().sin_cos();
    let (vx, vy) = (-w / 2.0, -h / 2.0);
    [
        centre[0] + vx * cos - vy * sin,
        centre[1] + vx * sin + vy * cos,
    ]
}

fn near(a: [f64; 2], b: [f64; 2], tolerance: f64) -> bool {
    (a[0] - b[0]).abs() < tolerance && (a[1] - b[1]).abs() < tolerance
}

const CLICK_TIP: [f64; 2] = [0.8 * 1920.0, 0.7 * 1080.0];

#[test]
fn cursor_motion_through_a_click_and_a_jump() {
    let p = project();
    let moments = [
        0.5, 0.6, 0.75, 0.9, 1.0, 1.05, 1.2, 1.3, 1.375, 1.55, 2.5, 2.52, 2.6, 2.8,
    ];
    let sprites: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "cursor": sprite_at(&p, CursorSettings::default(), t) }))
        .collect();
    insta::assert_json_snapshot!(sprites);
}

#[test]
fn click_squeezes_the_cursor_towards_its_hotspot() {
    let p = project();
    let bouncy = CursorSettings {
        bounce: 2.0,
        ..exact()
    };
    let calm = sprite_at(&p, exact(), 1.375).unwrap();
    // The click is at 1.2 s; half of 350 ms later the squeeze is deepest: 1 − 0.08 · 2.
    let squeezed = sprite_at(&p, bouncy.clone(), 1.375).unwrap();
    assert!((squeezed.rect.h / calm.rect.h - 0.84).abs() < 1e-4);
    assert!(near(arrow_tip(&squeezed), CLICK_TIP, 1e-3));
    assert_eq!(sprite_at(&p, bouncy.clone(), 1.2).unwrap().rect, calm.rect);
    assert_eq!(sprite_at(&p, bouncy.clone(), 1.56).unwrap().rect, calm.rect);
    let deep = CursorSettings {
        bounce: 5.0,
        ..bouncy
    };
    let floor = sprite_at(&p, deep, 1.375).unwrap();
    assert!((floor.rect.h / calm.rect.h - 0.72).abs() < 1e-4);
}

#[test]
fn bounce_runs_at_normal_speed_on_a_sped_up_part() {
    let mut p = project();
    p.clips[0].speeds = vec![SpeedRange {
        range: Range::new(s(1.0), s(1.6)).unwrap(),
        factor: 2.0,
    }];
    let bouncy = CursorSettings {
        bounce: 2.0,
        ..exact()
    };
    let calm = sprite_at(&p, exact(), 1.275).unwrap();
    // The click plays at output 1.1 s; 175 ms of output later, not of the recording.
    let squeezed = sprite_at(&p, bouncy, 1.275).unwrap();
    assert!((squeezed.rect.h / calm.rect.h - 0.84).abs() < 1e-4);
}

#[test]
fn sway_tilts_the_cursor_around_its_hotspot() {
    let p = project();
    let swaying = CursorSettings {
        sway: 2.0,
        ..exact()
    };
    let sprite = sprite_at(&p, swaying.clone(), 0.9).unwrap();
    // Moving right and down tilts clockwise.
    assert!(sprite.rotation > 10.0, "{}", sprite.rotation);
    let upright = sprite_at(&p, exact(), 0.9).unwrap();
    let tip = [f64::from(upright.rect.x), f64::from(upright.rect.y)];
    assert!(near(arrow_tip(&sprite), tip, 1e-2), "{sprite:?}");
    assert_eq!(sprite_at(&p, swaying, 2.4).unwrap().rotation, 0.0);
}

#[test]
fn motion_blur_trails_behind_a_moving_cursor_only() {
    let p = project();
    let blurred = CursorSettings {
        blur: 0.6,
        smoothing: Smoothing::Focused,
        ..exact()
    };
    let moving = sprite_at(&p, blurred.clone(), 0.75).unwrap();
    let from = moving.motion.unwrap().from;
    // 2300 px/s over 0.6 / 30 s of output: about 46 px back up the sweep.
    let (dx, dy) = (moving.rect.x - from[0], moving.rect.y - from[1]);
    assert!(dx > 25.0 && dx < 60.0 && dy > 0.0, "{dx} {dy}");
    // A sample every 2 px of the trail.
    let samples = moving.motion.unwrap().samples.get();
    let expected = f64::from(dx.hypot(dy)) / 2.0 + 1.0;
    assert!(
        (f64::from(samples) - expected).abs() <= 1.0,
        "{samples} {expected}"
    );
    assert_eq!(sprite_at(&p, blurred.clone(), 0.3).unwrap().motion, None);
    let off = CursorSettings {
        blur: 0.0,
        ..blurred
    };
    assert_eq!(sprite_at(&p, off, 0.75).unwrap().motion, None);
}

#[test]
fn motion_blur_does_not_reach_across_a_cut() {
    let mut p = project();
    // Output 0.6 jumps from the middle of the sweep (0.6 s) to 0.9 s.
    p.clips[0].cuts = vec![Range::new(s(0.6), s(0.9)).unwrap()];
    let blurred = CursorSettings {
        blur: 2.0,
        ..exact()
    };
    let sprite = sprite_at(&p, blurred, 0.61).unwrap();
    let dx = sprite.motion.map_or(0.0, |m| sprite.rect.x - m.from[0]);
    assert!(dx.abs() < 40.0, "{dx}");
}

#[test]
fn loop_brings_the_cursor_back_to_the_start() {
    let mut p = project();
    p.clips.truncate(1);
    p.clips[0].trim = Range::new(s(0.0), s(3.0)).unwrap();
    let looped = CursorSettings {
        looped: true,
        ..exact()
    };
    let end = TimeDelta::from_secs_f64(3.0).as_secs_f64();
    let tail = end - 0.67;
    let start = arrow_tip(&sprite_at(&p, exact(), 0.0).unwrap());
    assert!(near(start, [0.2 * 1920.0, 0.3 * 1080.0], 1e-3));
    let at = |secs| arrow_tip(&sprite_at(&p, looped.clone(), secs).unwrap());
    assert!(near(at(tail - 0.01), CLICK_TIP, 1e-3));
    assert!(near(at(tail), CLICK_TIP, 1e-3));
    let middle = at(tail + 0.1);
    assert!(middle[0] < CLICK_TIP[0] - 100.0 && middle[0] > start[0] + 100.0);
    assert!(near(at(tail + 0.55), start, 1e-3));
    assert!(near(at(end - 0.001), start, 1e-3));
    // Without loop the end shows where the recording is.
    let plain = arrow_tip(&sprite_at(&p, exact(), end - 0.001).unwrap());
    assert!(!near(plain, start, 10.0));
}
