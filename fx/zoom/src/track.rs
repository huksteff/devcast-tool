//! The camera track (docs/porting/zoom.md §5.2, decision 000): the camera of the whole video
//! simulated once with a fixed step, so the preview, the export and `devcast frame` show the
//! same frame at any frame rate and after any seek. `evaluate` reads it between two steps.

use std::num::NonZeroU32;

use dc_core::{CursorTracks, OutputTime, PixelSize, Point, Project, TimeDelta, TimeMap};
use dc_render::{Camera, Motion, Scene};

use crate::aim::Pulls;
use crate::follow::Follow;
use crate::model::{Mode, ZoomState};
use crate::pointer::Pointer;
use crate::shot::Shot;
use crate::spring::Spring;

/// Simulation step, seconds of the output video.
pub const STEP: f64 = 1.0 / 240.0;

/// Motion blur at strength 1 opens the shutter for this long (v1: the motion of 1/60 s, ×4 for
/// a long exposure); the strength scales it.
const SHUTTER: f64 = 4.0 / 60.0;
/// Samples of the camera blur (v1 kernel size).
const BLUR_SAMPLES: NonZeroU32 = NonZeroU32::MIN.saturating_add(12);

/// Consecutive simulation steps where the camera is not at rest.
#[derive(Debug, Clone, PartialEq)]
struct Run {
    first: usize,
    shots: Vec<[f32; 3]>,
}

/// The camera at every step of the output video.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraTrack {
    canvas: PixelSize,
    runs: Vec<Run>,
    /// How long the shutter of the camera blur is open; zero without blur.
    shutter: TimeDelta,
}

impl CameraTrack {
    /// Simulates the camera of `state` over the output of `map` on a `canvas`-sized frame of
    /// `project`; follow zooms follow the cursor of `cursors` (telemetry of the recordings).
    pub fn new(
        project: &Project,
        state: &ZoomState,
        map: &TimeMap,
        canvas: PixelSize,
        cursors: &CursorTracks,
    ) -> Self {
        let pointer = Pointer::new(project, map, cursors, canvas);
        Self::simulate(state, map, canvas, |t| pointer.at(t))
    }

    /// Simulates the camera of `state` over the output of `map` on a `canvas`-sized frame;
    /// `cursor` tells where the cursor is at an output second, fractions of the canvas.
    pub(crate) fn simulate(
        state: &ZoomState,
        map: &TimeMap,
        canvas: PixelSize,
        cursor: impl Fn(f64) -> Option<Point>,
    ) -> Self {
        let spring = Spring::new(state.settings.smoothness, STEP);
        let duration = map.duration().as_secs_f64();
        let limit = duration - spring.map_or(0.0, |s| s.settle());
        let pulls = Pulls::new(state, map, limit);
        let steps = (duration / STEP).ceil() as usize + 1;
        Self {
            canvas,
            runs: simulate(pulls, spring, steps, cursor),
            shutter: TimeDelta::from_secs_f64(SHUTTER * state.settings.motion_blur),
        }
    }

    /// The camera at output moment `t`, pixels of the canvas; at rest outside the video.
    pub fn camera(&self, t: OutputTime) -> Camera {
        let at = t.as_secs_f64() / STEP;
        let shot = if at < 0.0 {
            Shot::REST
        } else {
            let index = at.floor() as usize;
            self.shot(index)
                .lerp(self.shot(index + 1), at - index as f64)
        };
        Camera {
            scale: shot.scale as f32,
            x: (shot.x * f64::from(self.canvas.width)) as f32,
            y: (shot.y * f64::from(self.canvas.height)) as f32,
        }
    }

    /// Puts the camera of output moment `t` into `scene`, smeared over its path during the
    /// shutter (§9: the video only; zoom and pan alike) if it moved.
    pub fn evaluate(&self, t: OutputTime, scene: &mut Scene) {
        scene.camera = self.camera(t);
        let from = self.camera((t - self.shutter).max(OutputTime::ZERO));
        scene.camera_motion = (from != scene.camera).then_some(Motion {
            from,
            samples: BLUR_SAMPLES,
        });
    }

    fn shot(&self, step: usize) -> Shot {
        let run = self.runs.partition_point(|r| r.first <= step);
        let Some(run) = run.checked_sub(1).map(|i| &self.runs[i]) else {
            return Shot::REST;
        };
        run.shots
            .get(step - run.first)
            .map_or(Shot::REST, |&[scale, x, y]| Shot {
                scale: f64::from(scale),
                x: f64::from(x),
                y: f64::from(y),
            })
    }
}

/// Where the camera is pulled at `t`: towards the leading zoom, at the focus it follows.
fn aim(
    pulls: &mut Pulls<'_>,
    follow: &mut Follow,
    t: f64,
    cursor: &impl Fn(f64) -> Option<Point>,
) -> Shot {
    let Some((index, strength)) = pulls.lead(t) else {
        follow.reset();
        return Shot::REST;
    };
    let zoom = pulls.get(index).zoom;
    let pointer = if zoom.mode == Mode::Follow {
        cursor(t)
    } else {
        None
    };
    let focus = follow.focus(pulls.get(index).chain, zoom, strength, pointer);
    Shot::toward(zoom.scale, focus, strength)
}

fn simulate(
    mut pulls: Pulls<'_>,
    spring: Option<Spring>,
    steps: usize,
    cursor: impl Fn(f64) -> Option<Point>,
) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut follow = Follow::default();
    let mut shot = Shot::REST;
    let mut speed = Shot::STILL;
    for step in 0..steps {
        let t = step as f64 * STEP;
        let target = aim(&mut pulls, &mut follow, t, &cursor);
        match spring.filter(|_| step > 0) {
            Some(spring) => (shot, speed) = shot.chase(speed, target, &spring),
            None => shot = target,
        }
        if target == Shot::REST && shot.resting(speed) {
            shot = Shot::REST;
            speed = Shot::STILL;
            continue;
        }
        let covering = shot.covering();
        let sample = [covering.scale as f32, covering.x as f32, covering.y as f32];
        match runs.last_mut() {
            Some(run) if run.first + run.shots.len() == step => run.shots.push(sample),
            _ => runs.push(Run {
                first: step,
                shots: vec![sample],
            }),
        }
    }
    runs
}

#[cfg(test)]
#[path = "track_tests.rs"]
mod tests;
