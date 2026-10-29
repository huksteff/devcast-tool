//! The camera track (docs/porting/zoom.md §5.2, decision 000): the camera of the whole video
//! simulated once with a fixed step, so the preview, the export and `devcast frame` show the
//! same frame at any frame rate and after any seek. `evaluate` reads it between two steps.

use dc_core::{OutputTime, PixelSize, TimeMap};
use dc_render::{Camera, Scene};

use crate::frame::reachable;
use crate::model::{Zoom, ZoomState};
use crate::ramp::Ramp;
use crate::spring::{Axis, Spring};

/// Simulation step, seconds of the output video.
pub const STEP: f64 = 1.0 / 240.0;

/// Closer than this to the frame at rest (fractions of the frame, scale) with no zoom pulling,
/// the camera stops: 0.2 px of a 1080p frame.
const REST: f64 = 1e-4;
/// Slower than this (the same units per second) the camera may stop.
const REST_SPEED: f64 = 1e-3;

/// The camera in fractions of the frame: a point `p` (0..1) lands at `p·scale + (x, y)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shot {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}

impl Shot {
    pub(crate) const REST: Self = Self {
        scale: 1.0,
        x: 0.0,
        y: 0.0,
    };

    /// No motion: the speed of a camera at rest.
    const STILL: Self = Self {
        scale: 0.0,
        x: 0.0,
        y: 0.0,
    };

    /// The camera `strength` (0..1) of the way into `zoom`: its focus moves from where it is
    /// towards the centre of the frame (§4.5). The focus is kept far enough from the edges for
    /// the zoomed picture to cover the frame (§4.4).
    pub(crate) fn toward(zoom: &Zoom, strength: f64) -> Self {
        let scale = zoom.scale;
        let focus = reachable(zoom.focus, scale);
        Self {
            scale: 1.0 + (scale - 1.0) * strength,
            x: (0.5 - focus.x * scale) * strength,
            y: (0.5 - focus.y * scale) * strength,
        }
    }

    /// Moved and scaled so the canvas still covers the frame (a spring may overshoot it).
    fn covering(self) -> Self {
        let scale = self.scale.max(1.0);
        Self {
            scale,
            x: self.x.clamp(1.0 - scale, 0.0),
            y: self.y.clamp(1.0 - scale, 0.0),
        }
    }

    fn lerp(self, to: Self, k: f64) -> Self {
        Self {
            scale: self.scale + (to.scale - self.scale) * k,
            x: self.x + (to.x - self.x) * k,
            y: self.y + (to.y - self.y) * k,
        }
    }
}

/// A zoom on the output: its strength curve and the zoom it pulls towards.
struct Pull<'z> {
    ramp: Ramp,
    start: f64,
    zoom: &'z Zoom,
}

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
}

impl CameraTrack {
    /// Simulates the camera of `state` over the output of `map` on a `canvas`-sized frame.
    pub fn new(state: &ZoomState, map: &TimeMap, canvas: PixelSize) -> Self {
        let settings = &state.settings;
        let spring = Spring::new(settings.smoothness, STEP);
        let duration = map.duration().as_secs_f64();
        let limit = duration - spring.map_or(0.0, |s| s.settle());
        let (enter, exit) = (
            settings.zoom_in.as_secs_f64(),
            settings.zoom_out.as_secs_f64(),
        );
        let mut pulls: Vec<Pull<'_>> = state
            .zooms
            .iter()
            .filter_map(|zoom| {
                let span = map.output_span(&zoom.media, zoom.range)?;
                let (start, end) = (span.start().as_secs_f64(), span.end().as_secs_f64());
                Some(Pull {
                    ramp: Ramp::new(start, end, enter, exit, limit),
                    start,
                    zoom,
                })
            })
            .collect();
        pulls.sort_by(|a, b| a.ramp.window().0.total_cmp(&b.ramp.window().0));
        let runs = simulate(&pulls, spring, (duration / STEP).ceil() as usize + 1);
        Self { canvas, runs }
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

    /// Puts the camera of output moment `t` into `scene`.
    pub fn evaluate(&self, t: OutputTime, scene: &mut Scene) {
        scene.camera = self.camera(t);
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

/// Where the zooms pull the camera at `t`: the strongest zoom, the later one of equals.
fn target(pulls: &[Pull<'_>], t: f64) -> Shot {
    pulls
        .iter()
        .take_while(|p| p.ramp.window().0 <= t)
        .map(|p| (p.ramp.strength(t), p))
        .filter(|(strength, _)| *strength > 0.0)
        .max_by(|(a, p), (b, q)| a.total_cmp(b).then(p.start.total_cmp(&q.start)))
        .map_or(Shot::REST, |(strength, p)| Shot::toward(p.zoom, strength))
}

fn simulate(pulls: &[Pull<'_>], spring: Option<Spring>, steps: usize) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut shot = target(pulls, 0.0);
    let mut speed = Shot::STILL;
    let mut lo = 0;
    for step in 0..steps {
        let t = step as f64 * STEP;
        while lo < pulls.len() && pulls[lo].ramp.window().1 <= t {
            lo += 1;
        }
        let aim = target(&pulls[lo..], t);
        if let (Some(spring), true) = (spring, step > 0) {
            (shot, speed) = follow(&spring, shot, speed, aim);
        } else {
            shot = aim;
        }
        if aim == Shot::REST && resting(shot, speed) {
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

/// One spring step of every axis of `shot` towards `aim`.
fn follow(spring: &Spring, shot: Shot, speed: Shot, aim: Shot) -> (Shot, Shot) {
    let axis = |value: f64, speed: f64, aim: f64| {
        let next = spring.step(Axis {
            offset: value - aim,
            speed,
        });
        (aim + next.offset, next.speed)
    };
    let (scale, scale_speed) = axis(shot.scale, speed.scale, aim.scale);
    let (x, x_speed) = axis(shot.x, speed.x, aim.x);
    let (y, y_speed) = axis(shot.y, speed.y, aim.y);
    (
        Shot { scale, x, y },
        Shot {
            scale: scale_speed,
            x: x_speed,
            y: y_speed,
        },
    )
}

fn resting(shot: Shot, speed: Shot) -> bool {
    let near = (shot.scale - 1.0).abs().max(shot.x.abs()).max(shot.y.abs());
    let slow = speed.scale.abs().max(speed.x.abs()).max(speed.y.abs());
    near < REST && slow < REST_SPEED
}

#[cfg(test)]
#[path = "track_tests.rs"]
mod tests;
