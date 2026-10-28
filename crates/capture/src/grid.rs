//! Frame times → slots of the recording frame rate (decision 007).
//!
//! Capture sends a frame only when the picture changes, so the video has variable frame rate:
//! a frame gets the nearest slot `round((time − start) × fps)`, a frame whose slot is already
//! taken is dropped (more changes than the frame rate), and a frame lasts until the next one.
//! Nothing is repeated while the screen stands still.

use crate::config::Fps;
use crate::event::HostTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGrid {
    /// Time of slot 0: the first frame.
    start: HostTime,
    fps: i64,
}

impl FrameGrid {
    pub fn new(start: HostTime, fps: Fps) -> Self {
        Self {
            start,
            fps: i64::from(fps.get()),
        }
    }

    pub fn start(&self) -> HostTime {
        self.start
    }

    /// The slot of a frame shown at `time`, or `None` when the frame is not needed: its slot is
    /// not after `last`, the slot of the previous kept frame.
    pub fn place(&self, last: Option<i64>, time: HostTime) -> Option<i64> {
        let slot = self.slot_at(time);
        last.is_none_or(|last| slot > last).then_some(slot)
    }

    /// The slot where a recording stopped at `time` ends: the last frame (slot `last`) lasts at
    /// least one slot.
    pub fn end(&self, last: i64, time: HostTime) -> i64 {
        self.slot_at(time).max(last + 1)
    }

    fn slot_at(&self, time: HostTime) -> i64 {
        let micros = time.micros() - self.start.micros();
        (micros * self.fps + 500_000).div_euclid(1_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: f64) -> HostTime {
        HostTime::from_micros(1_000_000_000 + (ms * 1000.0) as i64)
    }

    fn place_all(grid: FrameGrid, times: &[f64]) -> Vec<Option<i64>> {
        let mut last = None;
        times
            .iter()
            .map(|t| {
                let slot = grid.place(last, at(*t));
                last = slot.or(last);
                slot
            })
            .collect()
    }

    #[test]
    fn monitor_rate_frames_fill_consecutive_slots_despite_jitter() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps60);
        let slots = place_all(grid, &[0.0, 16.9, 33.1, 50.2, 66.4]);
        assert_eq!(slots, [Some(0), Some(1), Some(2), Some(3), Some(4)]);
    }

    #[test]
    fn still_screen_leaves_a_gap() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps30);
        assert_eq!(place_all(grid, &[0.0, 2000.0]), [Some(0), Some(60)]);
        assert_eq!(grid.end(60, at(5000.0)), 150);
    }

    #[test]
    fn extra_frames_are_dropped() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps30);
        let slots = place_all(grid, &[0.0, 16.7, 33.3, 50.0, 66.7, 60.0]);
        assert_eq!(slots, [Some(0), Some(1), None, Some(2), None, None]);
    }

    #[test]
    fn high_rate_on_a_slower_monitor_skips_slots() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps120);
        let slots = place_all(grid, &[0.0, 16.7, 33.3]);
        assert_eq!(slots, [Some(0), Some(2), Some(4)]);
    }

    #[test]
    fn end_is_after_the_last_frame_even_if_stop_comes_earlier() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps60);
        // Frame times are when the frame is shown, which can be ahead of the clock.
        assert_eq!(grid.end(60, at(980.0)), 61);
        assert_eq!(grid.end(60, at(1500.0)), 90);
    }
}
