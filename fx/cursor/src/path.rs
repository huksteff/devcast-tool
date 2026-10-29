//! Where the drawn cursor is at every moment of a recording (docs/porting/cursor.md §5.5): the
//! spring runs once over the whole recording with a fixed step from its first mouse event, so the
//! preview, the export, a paused frame and any fps show the same path, and cuts and speed do not
//! change it.

use dc_core::{CursorPoint, Point, SourceTime, TimeDelta};

use crate::spring::{Axis, Spring};

/// Steps of the spring per second of the recording.
const RATE: f64 = 240.0;

/// The cursor path of one recording, in fractions of its picture.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Path {
    start: SourceTime,
    /// The pointer as recorded: it stays at an event until the next one.
    moves: Vec<CursorPoint>,
    /// Smoothed positions every `1 / RATE` s from `start`; empty for the exact motion.
    smooth: Vec<[f64; 2]>,
}

impl Path {
    /// The path over `0..length` of a recording with these mouse events (at least one); without
    /// a spring it is exactly the recorded one.
    pub fn new(moves: Vec<CursorPoint>, spring: Option<&Spring>, length: TimeDelta) -> Self {
        let start = moves.first().map_or(SourceTime::ZERO, |m| m.at);
        let mut path = Self {
            start,
            moves,
            smooth: Vec::new(),
        };
        if let Some(spring) = spring {
            path.smooth = path.smoothed(spring, length);
        }
        path
    }

    fn smoothed(&self, spring: &Spring, length: TimeDelta) -> Vec<[f64; 2]> {
        let steps = ((SourceTime::ZERO + length).since(self.start).as_secs_f64() * RATE).ceil();
        let first = self.recorded(self.start);
        let (mut x, mut y) = (Axis::at(first.x), Axis::at(first.y));
        let mut out = vec![[first.x, first.y]];
        for i in 1..=steps as i64 {
            let target = self.recorded(self.step_time(i - 1));
            spring.step(&mut x, target.x, 1.0 / RATE);
            spring.step(&mut y, target.y, 1.0 / RATE);
            out.push([x.x, y.x]);
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
        let Some(last) = self.smooth.len().checked_sub(1) else {
            return self.recorded(t);
        };
        let f = (t.since(self.start).as_secs_f64() * RATE).clamp(0.0, last as f64);
        let i = (f.floor() as usize).min(last.saturating_sub(1));
        let (a, b) = (self.smooth[i], self.smooth[(i + 1).min(last)]);
        let s = f - i as f64;
        Point {
            x: a[0] + (b[0] - a[0]) * s,
            y: a[1] + (b[1] - a[1]) * s,
        }
    }
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
