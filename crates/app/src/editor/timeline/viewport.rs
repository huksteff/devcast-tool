//! What part of the project the timeline lanes show: the time scale, the time at the left edge
//! of the lanes and the height factor of the rows. Pure math over the lanes of a frame; the
//! timeline (`pointer.rs`) feeds it the wheel, the slider, the navigation bar and the drags.
//!
//! Until the user zooms, the whole project fits the lanes and follows the window and the project
//! length. Zooming out to the whole project goes back to that.

use std::ops::RangeInclusive;

use dc_core::{OutputTime, TimeDelta};

use super::scale::TimeScale;

/// The finest scale shows this many frames per 100 points.
const FINEST_FRAMES_PER_100: f64 = 4.0;
/// The length an empty project is shown with, seconds.
const EMPTY: f64 = 1.0;
/// Row height factor (Shift+wheel, v1): limits and the step of one wheel notch.
pub const ROWS: RangeInclusive<f32> = 0.5..=3.0;
const ROWS_STEP: f32 = 0.1;
/// Points of wheel in one notch (egui: a line is 40 points).
const NOTCH: f32 = 40.0;
/// Auto-scroll near the edges of the lanes: the zone, points, and the speed at the edge, visible
/// spans per second (v1: 0.018 of the span per frame at 60 fps).
const EDGE: f32 = 32.0;
const EDGE_SPEED: f64 = 1.08;

/// The lanes of a frame: from x `left`, `width` points wide; the project's output length and the
/// frame rate of its clips.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lanes {
    pub left: f32,
    pub width: f32,
    pub duration: TimeDelta,
    pub fps: f64,
}

impl Lanes {
    pub fn right(&self) -> f32 {
        self.left + self.width
    }

    /// Output length, seconds (an empty project has one).
    fn length(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs > 0.0 { secs } else { EMPTY }
    }

    /// Points per second: the whole project (least) and a few frames per 100 points (most).
    pub fn limits(&self) -> (f64, f64) {
        let whole = f64::from(self.width.max(1.0)) / self.length();
        let finest = 100.0 * self.fps / FINEST_FRAMES_PER_100;
        (whole, finest.max(whole))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Viewport {
    /// Points per second; `None` — the whole project fits the lanes.
    zoom: Option<f64>,
    /// Output time at the left edge of the lanes, seconds; kept in range by `settle`.
    start: f64,
    /// Row height factor as the wheel moved it, unrounded; `None` until the settings give it.
    rows: Option<f32>,
    /// The playhead the view saw last: a playhead that moved by itself is brought into view.
    seen: Option<OutputTime>,
    /// While the playhead is dragged: whether playback goes on after the release.
    pub scrub: Option<bool>,
    /// While the navigation thumb is dragged: the pointer x and the start when it was grabbed.
    pub grab: Option<(f32, f64)>,
    /// The last wheel notch came with Shift: its smoothed rest scales the rows too.
    pub shift_wheel: bool,
}

impl Viewport {
    /// Points per second now.
    pub fn px_per_sec(&self, lanes: &Lanes) -> f64 {
        let (least, most) = lanes.limits();
        self.zoom.map_or(least, |z| z.clamp(least, most))
    }

    /// Seconds the lanes show.
    pub fn visible(&self, lanes: &Lanes) -> f64 {
        f64::from(lanes.width.max(1.0)) / self.px_per_sec(lanes)
    }

    fn last_start(&self, lanes: &Lanes) -> f64 {
        (lanes.length() - self.visible(lanes)).max(0.0)
    }

    /// Output time at the left edge, seconds.
    pub fn start(&self, lanes: &Lanes) -> f64 {
        self.start.clamp(0.0, self.last_start(lanes))
    }

    pub fn scale(&self, lanes: &Lanes) -> TimeScale {
        TimeScale {
            left: lanes.left,
            start: OutputTime::from_secs_f64(self.start(lanes)),
            px_per_sec: self.px_per_sec(lanes) as f32,
        }
    }

    /// Keeps the zoom and the start in range of `lanes` (the window or the project changed).
    pub fn settle(&mut self, lanes: &Lanes) {
        if let Some(zoom) = self.zoom {
            let (least, most) = lanes.limits();
            self.zoom = (zoom > least * (1.0 + 1e-9)).then_some(zoom.min(most));
        }
        self.start = self.start(lanes);
    }

    /// The whole project in the lanes («↔»).
    pub fn fit(&mut self) {
        self.zoom = None;
        self.start = 0.0;
    }

    /// `px_per_sec` (clamped), keeping the time at x `anchor` where it is.
    pub fn zoom_to(&mut self, lanes: &Lanes, px_per_sec: f64, anchor: f32) {
        let x = anchor.clamp(lanes.left, lanes.right());
        let at = self.start(lanes) + f64::from(x - lanes.left) / self.px_per_sec(lanes);
        self.zoom = Some(px_per_sec);
        self.settle(lanes);
        self.start = at - f64::from(x - lanes.left) / self.px_per_sec(lanes);
        self.settle(lanes);
    }

    /// Scales by `factor` around x `anchor` (Ctrl+wheel around the pointer).
    pub fn zoom_by(&mut self, lanes: &Lanes, factor: f64, anchor: f32) {
        let px = self.px_per_sec(lanes) * factor;
        self.zoom_to(lanes, px, anchor);
    }

    /// The scale on the slider: 0 is the whole project, 1 the finest, logarithmic between.
    pub fn slider(&self, lanes: &Lanes) -> f64 {
        let (least, most) = lanes.limits();
        if most <= least {
            return 0.0;
        }
        (self.px_per_sec(lanes) / least).ln() / (most / least).ln()
    }

    /// Sets the scale from the slider `value` (0..1) around x `anchor`.
    pub fn set_slider(&mut self, lanes: &Lanes, value: f64, anchor: f32) {
        let (least, most) = lanes.limits();
        let px = least * (most / least).powf(value.clamp(0.0, 1.0));
        self.zoom_to(lanes, px, anchor);
    }

    /// Moves the content `dx` points to the right (the wheel's direction): earlier time comes in.
    pub fn scroll(&mut self, lanes: &Lanes, dx: f32) {
        self.start = self.start(lanes) - f64::from(dx) / self.px_per_sec(lanes);
        self.settle(lanes);
    }

    /// Start and length of the visible time as shares of the project (the navigation thumb).
    pub fn thumb(&self, lanes: &Lanes) -> (f64, f64) {
        let length = lanes.length();
        let share = (self.visible(lanes) / length).min(1.0);
        (self.start(lanes) / length, share)
    }

    /// Puts the middle of the visible time at `share` of the project (a click on the bar).
    pub fn centre_at(&mut self, lanes: &Lanes, share: f64) {
        self.start = share * lanes.length() - self.visible(lanes) / 2.0;
        self.settle(lanes);
    }

    /// The visible time starts at `start` moved by `share` of the project (a dragged thumb).
    pub fn shift_from(&mut self, lanes: &Lanes, start: f64, share: f64) {
        self.start = start + share * lanes.length();
        self.settle(lanes);
    }

    /// Scrolls towards the edge the pointer at `x` presses for `dt` seconds; false when it is
    /// in neither zone or there is nothing to scroll.
    pub fn edge_scroll(&mut self, lanes: &Lanes, x: f32, dt: f32) -> bool {
        let press = if x < lanes.left + EDGE {
            -((lanes.left + EDGE - x) / EDGE).min(1.0)
        } else if x > lanes.right() - EDGE {
            ((x - lanes.right() + EDGE) / EDGE).min(1.0)
        } else {
            0.0
        };
        if press == 0.0 || self.last_start(lanes) <= 0.0 {
            return false;
        }
        let step = f64::from(press) * self.visible(lanes) * EDGE_SPEED * f64::from(dt);
        self.start = self.start(lanes) + step;
        self.settle(lanes);
        true
    }

    /// A playhead that moved by itself (playback, keys) and left the view pages the view to it;
    /// a dragged one is left to the auto-scroll.
    pub fn follow(&mut self, lanes: &Lanes, playhead: OutputTime) {
        if self.seen == Some(playhead) {
            return;
        }
        self.seen = Some(playhead);
        let t = playhead.as_secs_f64();
        let start = self.start(lanes);
        let outside = t < start || t > start + self.visible(lanes);
        if outside && self.scrub.is_none() {
            self.start = t;
            self.settle(lanes);
        }
    }

    /// The row height factor, in steps of a notch; `saved` (the settings) until the wheel moves.
    pub fn rows(&mut self, saved: f32) -> f32 {
        let raw = *self
            .rows
            .get_or_insert_with(|| saved.clamp(*ROWS.start(), *ROWS.end()));
        ((raw / ROWS_STEP).round() * ROWS_STEP).clamp(*ROWS.start(), *ROWS.end())
    }

    /// The wheel moved `delta` points with Shift: up is taller.
    pub fn wheel_rows(&mut self, saved: f32, delta: f32) {
        self.rows(saved);
        if let Some(raw) = &mut self.rows {
            *raw = (*raw + delta / NOTCH * ROWS_STEP).clamp(*ROWS.start(), *ROWS.end());
        }
    }
}
