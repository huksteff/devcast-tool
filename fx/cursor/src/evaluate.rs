//! The cursor in the scene: the picture the system showed during the recording («Системный»
//! set) at the drawn path, scaled to the clip, squeezed by clicks, tilted by sway and smeared by
//! motion blur (docs/porting/cursor.md §4, §7–§11).

use std::f64::consts::PI;
use std::num::NonZeroU32;

use dc_core::{OutputTime, Project, TimeDelta, TimeMap};
use dc_render::{Band, ImageKey, Layer, Motion, PxRect, Scene, Sprite};

use crate::clicks;
use crate::pose::{Pose, pose_at};
use crate::prepared::Prepared;
use crate::state::CursorSettings;

/// Picture height over the height of the arrow in it: a Windows arrow is 19.0625 px of its 32 px
/// box (v1 `k_style`), so every picture is drawn this many times taller than `h`.
const PICTURE_PER_ARROW: f64 = 32.0 / 19.0625;
/// v1 `dotAlpha`.
const OPACITY: f32 = 0.95;
/// Motion blur shows the trail of `blur ×` this much output time (v1: 2 × 1/60 s).
const BLUR_TRAIL: f64 = 1.0 / 30.0;
/// Motion blur takes a sample every this many px of the trail, so a long trail does not fall
/// apart into separate cursors (v1 had 13 samples at any length and showed them).
const BLUR_SAMPLE_STEP: f64 = 2.0;
/// The most samples a trail gets: the shader reads the picture this many times per pixel.
const BLUR_SAMPLES_MAX: u32 = 64;
/// A shorter trail, px of the frame, is not blurred (v1).
const BLUR_MIN: f64 = 0.01;
/// Squeeze per unit of bounce and its floor (v1).
const BOUNCE_DEPTH: f64 = 0.08;
const BOUNCE_FLOOR: f64 = 0.72;

/// Puts the cursor of output moment `t` into `scene`, over everything under the camera.
pub fn evaluate(
    prepared: &Prepared,
    project: &Project,
    map: &TimeMap,
    t: OutputTime,
    scene: &mut Scene,
) {
    let settings = &prepared.settings;
    if !settings.show {
        return;
    }
    let Some(pose) = pose_at(prepared, project, map, t, scene.size) else {
        return;
    };
    let squeeze = prepared
        .place(project, map, t, scene.size)
        .and_then(|place| clicks::recent(&place, map, t, settings.bounce_time).next())
        .map_or(1.0, |(_, age)| bounce(settings, age));
    let mut sprite = sprite(&pose, squeeze);
    sprite.motion = blur(prepared, project, map, t, scene, &pose, &sprite.rect);
    scene.push(Band::Cursor, Layer::Sprite(sprite));
}

/// Height factor of the cursor `age` after a click: 1 → `1 − 0.08 · bounce` at half the time → 1.
fn bounce(settings: &CursorSettings, age: TimeDelta) -> f64 {
    let p = 1.0 - age.as_secs_f64() / settings.bounce_time.as_secs_f64();
    (1.0 - (p * PI).sin() * BOUNCE_DEPTH * settings.bounce).max(BOUNCE_FLOOR)
}

/// The trail from where the cursor was `blur / 30` s of output ago, not across a cut.
fn blur(
    prepared: &Prepared,
    project: &Project,
    map: &TimeMap,
    t: OutputTime,
    scene: &Scene,
    pose: &Pose<'_>,
    rect: &PxRect,
) -> Option<Motion<[f32; 2]>> {
    let blur = prepared.settings.blur;
    let start = prepared.place(project, map, t, scene.size)?.start;
    let back = (t - TimeDelta::from_secs_f64(blur * BLUR_TRAIL)).max(start);
    let then = pose_at(prepared, project, map, back, scene.size)?;
    let (dx, dy) = (then.tip[0] - pose.tip[0], then.tip[1] - pose.tip[1]);
    let trail = dx.hypot(dy);
    let samples = ((trail / BLUR_SAMPLE_STEP).ceil() as u32 + 1).min(BLUR_SAMPLES_MAX);
    (blur > 0.0 && trail >= BLUR_MIN).then(|| Motion {
        from: [rect.x + dx as f32, rect.y + dy as f32],
        samples: NonZeroU32::MIN.saturating_add(samples - 1),
    })
}

/// The picture of `pose` with its hotspot on the tip, `squeeze` times its height, tilted around
/// the hotspot.
fn sprite(pose: &Pose<'_>, squeeze: f64) -> Sprite {
    let picture = pose.picture;
    let (pw, ph) = (
        f64::from(picture.size.width),
        f64::from(picture.size.height),
    );
    let h = pose.arrow * PICTURE_PER_ARROW * squeeze;
    let w = h * pw / ph;
    // From the hotspot to the centre, turned with the picture.
    let (vx, vy) = (
        w / 2.0 - picture.hotspot.x / pw * w,
        h / 2.0 - picture.hotspot.y / ph * h,
    );
    let (sin, cos) = pose.turn.sin_cos();
    let centre = [
        pose.tip[0] + vx * cos - vy * sin,
        pose.tip[1] + vx * sin + vy * cos,
    ];
    let rect = PxRect {
        x: (centre[0] - w / 2.0) as f32,
        y: (centre[1] - h / 2.0) as f32,
        w: w as f32,
        h: h as f32,
    };
    Sprite {
        image: ImageKey {
            path: picture.path.clone(),
            frame: 0,
        },
        rect,
        rotation: pose.turn.to_degrees() as f32,
        opacity: OPACITY,
        motion: None,
    }
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "motion_tests.rs"]
mod motion_tests;
