//! Output time ↔ x on the timeline lanes, the ruler steps and the time labels.

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};

/// Labelled ruler steps, seconds; the ruler takes the first one wide enough.
const STEPS: [i64; 12] = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600];
/// Least distance between two labelled ticks, points (a «10:00» label and air).
const MIN_LABEL_GAP: f32 = 56.0;
/// The scale of an empty project: one second over the whole width.
const EMPTY: TimeDelta = TimeDelta::from_micros(1_000_000);

/// Where output time is on screen: `start` is at x `left`, `px_per_sec` points per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScale {
    pub left: f32,
    pub start: OutputTime,
    pub px_per_sec: f32,
}

impl TimeScale {
    /// The whole `duration` in `width` points from `left`.
    pub fn fit(left: f32, width: f32, duration: TimeDelta) -> Self {
        let duration = if duration > TimeDelta::ZERO {
            duration
        } else {
            EMPTY
        };
        Self {
            left,
            start: OutputTime::ZERO,
            px_per_sec: width.max(1.0) / duration.as_secs_f64() as f32,
        }
    }

    pub fn x(&self, t: OutputTime) -> f32 {
        self.left + t.since(self.start).as_secs_f64() as f32 * self.px_per_sec
    }

    /// Output time at `x`, never before zero.
    pub fn time(&self, x: f32) -> OutputTime {
        let secs = f64::from((x - self.left) / self.px_per_sec);
        let t = self.start + TimeDelta::from_secs_f64(secs);
        t.max(OutputTime::ZERO)
    }

    /// Left and right x of `range`.
    pub fn span(&self, range: Range<Output>) -> (f32, f32) {
        (self.x(range.start()), self.x(range.end()))
    }

    /// Distance between labelled ticks; a minor tick stands halfway.
    pub fn tick_step(&self) -> TimeDelta {
        let seconds = STEPS
            .iter()
            .copied()
            .find(|&s| s as f32 * self.px_per_sec >= MIN_LABEL_GAP)
            .unwrap_or(STEPS[STEPS.len() - 1]);
        TimeDelta::from_micros(seconds * 1_000_000)
    }
}

/// «0:33.40»: minutes, seconds and hundredths; hours in front from one hour on.
pub fn timecode(t: OutputTime) -> String {
    let hundredths = t.micros().max(0) / 10_000;
    let (seconds, cc) = (hundredths / 100, hundredths % 100);
    format!("{}.{cc:02}", ruler_label(seconds))
}

/// «0:52.0»: a length with tenths of a second.
pub fn length(length: TimeDelta) -> String {
    let tenths = length.micros().max(0) / 100_000;
    format!("{}.{}", ruler_label(tenths / 10), tenths % 10)
}

/// «1:10» of a ruler tick at `seconds`.
pub fn ruler_label(seconds: i64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: f64) -> OutputTime {
        OutputTime::from_secs_f64(secs)
    }

    #[test]
    fn fit_puts_the_whole_project_into_the_width() {
        let scale = TimeScale::fit(148.0, 1000.0, TimeDelta::from_secs_f64(100.0));
        assert_eq!(scale.x(OutputTime::ZERO), 148.0);
        assert_eq!(scale.x(at(100.0)), 1148.0);
        assert_eq!(scale.x(at(33.4)), 482.0);
    }

    #[test]
    fn x_and_time_are_inverse() {
        let scale = TimeScale::fit(148.0, 900.0, TimeDelta::from_secs_f64(108.0));
        for secs in [0.0, 0.5, 33.4, 107.99] {
            let back = scale.time(scale.x(at(secs)));
            assert!(
                (back.as_secs_f64() - secs).abs() < 1e-3,
                "{secs} → {back:?}"
            );
        }
        assert_eq!(scale.time(10.0), OutputTime::ZERO, "left of the lanes is 0");
    }

    #[test]
    fn scrolled_scale_starts_at_its_start() {
        let scale = TimeScale {
            left: 0.0,
            start: at(10.0),
            px_per_sec: 20.0,
        };
        assert_eq!(scale.x(at(10.0)), 0.0);
        assert_eq!(scale.x(at(12.5)), 50.0);
        assert_eq!(scale.time(100.0), at(15.0));
    }

    #[test]
    fn empty_project_still_has_a_scale() {
        let scale = TimeScale::fit(0.0, 500.0, TimeDelta::ZERO);
        assert_eq!(scale.px_per_sec, 500.0);
    }

    #[test]
    fn ticks_keep_labels_apart() {
        let step = |width: f32, secs: f64| {
            TimeScale::fit(0.0, width, TimeDelta::from_secs_f64(secs))
                .tick_step()
                .as_secs_f64()
        };
        assert_eq!(step(1132.0, 108.0), 10.0);
        assert_eq!(step(1132.0, 10.0), 1.0);
        assert_eq!(step(1132.0, 3600.0), 300.0);
        assert_eq!(step(100.0, 360_000.0), 3600.0, "the largest step at most");
    }

    #[test]
    fn time_labels() {
        assert_eq!(timecode(at(33.4)), "0:33.40");
        assert_eq!(timecode(at(108.0)), "1:48.00");
        assert_eq!(timecode(at(3725.05)), "1:02:05.05");
        assert_eq!(ruler_label(70), "1:10");
        assert_eq!(ruler_label(0), "0:00");
        assert_eq!(length(TimeDelta::from_secs_f64(52.04)), "0:52.0");
    }
}
