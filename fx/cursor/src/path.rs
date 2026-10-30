//! Where the drawn cursor is and how it tilts at every moment of a recording
//! (docs/porting/cursor.md §5.5, §10): the springs run once over the whole recording with a fixed
//! step from its first mouse event, so the preview, the export, a paused frame and any fps show
//! the same path, and cuts and speed do not change it.

use std::f64::consts::PI;

use dc_core::{CursorPoint, Point, SourceTime, TimeDelta};

use crate::prepared::REFERENCE_WIDTH;
use crate::spring::{Axis, Spring};

/// Steps of the springs per second of the recording.
const RATE: f64 = 240.0;
/// Speed in ref-px (px of a picture 1920 px wide) per second at which the tilt is full (v1).
const FULL_TILT_SPEED: f64 = 1400.0;
/// Shorter moves in ref-px do not tilt (v1).
const STILL: f64 = 0.01;

/// The tilt in the direction of motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sway {
    pub spring: Spring,
    /// `CursorSettings::sway`, > 0.
    pub strength: f64,
}

/// The cursor path of one recording, in fractions of its picture.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Path {
    start: SourceTime,
    /// The pointer as recorded: it stays at an event until the next one.
    moves: Vec<CursorPoint>,
    smoothed: bool,
    /// x, y and the tilt in radians (clockwise) every `1 / RATE` s from `start`; empty for the
    /// exact motion without sway.
    steps: Vec<[f64; 3]>,
}

impl Path {
    /// The path over `0..length` of a recording with these mouse events (at least one) and a
    /// picture of `aspect` (height / width); without a spring it is exactly the recorded one.
    pub fn new(
        moves: Vec<CursorPoint>,
        spring: Option<&Spring>,
        sway: Option<&Sway>,
        length: TimeDelta,
        aspect: f64,
    ) -> Self {
        let start = moves.first().map_or(SourceTime::ZERO, |m| m.at);
        let mut path = Self {
            start,
            moves,
            smoothed: spring.is_some(),
            steps: Vec::new(),
        };
        if spring.is_some() || sway.is_some() {
            path.steps = path.stepped(spring, sway, length, aspect);
        }
        path
    }

    fn stepped(
        &self,
        spring: Option<&Spring>,
        sway: Option<&Sway>,
        length: TimeDelta,
        aspect: f64,
    ) -> Vec<[f64; 3]> {
        let count = ((SourceTime::ZERO + length).since(self.start).as_secs_f64() * RATE).ceil();
        let dt = 1.0 / RATE;
        let first = self.recorded(self.start);
        let (mut x, mut y, mut turn) = (Axis::at(first.x), Axis::at(first.y), Axis::at(0.0));
        let mut out = vec![[first.x, first.y, 0.0]];
        let mut last = first;
        for i in 1..=count as i64 {
            let pos = match spring {
                Some(spring) => {
                    let target = self.recorded(self.step_time(i - 1));
                    spring.step(&mut x, target.x, dt);
                    spring.step(&mut y, target.y, dt);
                    Point { x: x.x, y: y.x }
                }
                None => self.recorded(self.step_time(i)),
            };
            if let Some(sway) = sway {
                let dx = (pos.x - last.x) * REFERENCE_WIDTH;
                let dy = (pos.y - last.y) * REFERENCE_WIDTH * aspect;
                sway.spring
                    .step(&mut turn, tilt(dx, dy, dt, sway.strength), dt);
            }
            out.push([pos.x, pos.y, turn.x]);
            last = pos;
        }
        out
    }

    fn step_time(&self, i: i64) -> SourceTime {
        self.start + TimeDelta::from_micros((i as f64 * 1e6 / RATE).round() as i64)
    }

    /// The recorded pointer at `t`: the last event not after it, or the first one.
    pub fn recorded(&self, t: SourceTime) -> Point {
        let index = self.moves.partition_point(|m| m.at <= t);
        self.moves[index.saturating_sub(1)].pos
    }

    /// The drawn cursor at `t`.
    pub fn at(&self, t: SourceTime) -> Point {
        match self.step_at(t) {
            Some([x, y, _]) if self.smoothed => Point { x, y },
            _ => self.recorded(t),
        }
    }

    /// The tilt at `t`, radians clockwise.
    pub fn turn(&self, t: SourceTime) -> f64 {
        self.step_at(t).map_or(0.0, |[_, _, turn]| turn)
    }

    fn step_at(&self, t: SourceTime) -> Option<[f64; 3]> {
        let last = self.steps.len().checked_sub(1)?;
        let f = (t.since(self.start).as_secs_f64() * RATE).clamp(0.0, last as f64);
        let i = (f.floor() as usize).min(last.saturating_sub(1));
        let (a, b) = (self.steps[i], self.steps[(i + 1).min(last)]);
        let s = f - i as f64;
        Some([0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * s))
    }
}

/// The tilt the cursor leans to after moving (`dx`, `dy`) ref-px in `dt` s (v1 `sway_target`):
/// right and down lean clockwise, full `strength · 30°` from `FULL_TILT_SPEED`.
fn tilt(dx: f64, dy: f64, dt: f64, strength: f64) -> f64 {
    let d = dx.hypot(dy);
    if d < STILL {
        return 0.0;
    }
    let speed = (d / dt / FULL_TILT_SPEED).min(1.0);
    let bias = ((dx + 0.65 * dy) / d).clamp(-1.0, 1.0);
    bias * speed * (PI / 18.0) * strength * 3.0
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
