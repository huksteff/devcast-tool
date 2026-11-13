//! Frame times → slots of the recording frame rate (decision 007).
//!
//! Capture sends a frame only when the picture changes, so the video has variable frame rate:
//! a frame gets the nearest slot `round((time − start) × fps)`. A frame rounded onto the slot of
//! the previous one but shown after that slot's moment takes the next slot (at most one slot
//! late): a 60 fps picture on a 75 Hz monitor reaches the screen 13 or 27 ms apart and keeps all
//! its frames. Other frames on a taken slot are dropped (more changes than the frame rate), and a
//! frame lasts until the next one.
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
    /// not after `last`, the slot of the previous kept frame, and it was not shown after `last`'s
    /// moment.
    pub fn place(&self, last: Option<i64>, time: HostTime) -> Option<i64> {
        let slot = self.slot_at(time);
        match last {
            Some(last) if slot <= last => self.after(time, last).then_some(last + 1),
            _ => Some(slot),
        }
    }

    /// The slot of `time` for a repeat of the last picture, or `None` when it is not after `last`:
    /// a repeat shows nothing new, so it never takes the next slot early.
    pub fn repeat(&self, last: Option<i64>, time: HostTime) -> Option<i64> {
        let slot = self.slot_at(time);
        last.is_none_or(|last| slot > last).then_some(slot)
    }

    /// Whether `time` is after the moment of `slot`.
    fn after(&self, time: HostTime, slot: i64) -> bool {
        (time.micros() - self.start.micros()) * self.fps > slot * 1_000_000
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
    fn a_60_fps_picture_on_a_75_hz_monitor_keeps_every_frame() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps60);
        // Shown on 13.3 ms vsyncs: 13.3, 13.3, 13.3, 26.7 ms apart.
        let slots = place_all(
            grid,
            &[0.0, 13.3, 26.7, 40.0, 66.7, 80.0, 93.3, 106.7, 133.3],
        );
        let kept: Vec<i64> = slots.into_iter().flatten().collect();
        assert_eq!(kept, [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_faster_picture_stays_less_than_a_slot_late() {
        let grid = FrameGrid::new(at(0.0), Fps::Fps60);
        let times: Vec<f64> = (0..240)
            .map(|i| f64::from(i) * 8.33 + f64::from(i % 3) * 0.4)
            .collect();
        let mut last = None;
        for t in times {
            if let Some(slot) = grid.place(last, at(t)) {
                let late = slot as f64 - t / (1000.0 / 60.0);
                assert!((-0.5..1.0).contains(&late), "{t} ms in slot {slot}");
                last = Some(slot);
            }
        }
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
        assert_eq!(slots, [Some(0), Some(1), None, Some(2), Some(3), None]);
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
