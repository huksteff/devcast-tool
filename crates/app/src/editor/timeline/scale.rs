//! Output time ↔ x on the timeline lanes, the ruler steps and the time labels.

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};

/// Labelled ruler steps, microseconds (v1); the ruler takes the first one wide enough.
const STEPS: [i64; 17] = [
    50_000,
    100_000,
    250_000,
    500_000,
    1_000_000,
    2_000_000,
    5_000_000,
    10_000_000,
    15_000_000,
    30_000_000,
    60_000_000,
    120_000_000,
    300_000_000,
    600_000_000,
    900_000_000,
    1_800_000_000,
    3_600_000_000,
];
/// Least distance between two labelled ticks, points (a «0:33.25» label and air).
const MIN_LABEL_GAP: f32 = 56.0;

/// Where output time is on screen: `start` is at x `left`, `px_per_sec` points per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScale {
    pub left: f32,
    pub start: OutputTime,
    pub px_per_sec: f32,
}

impl TimeScale {
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
        let micros = STEPS
            .iter()
            .copied()
            .find(|&s| s as f32 / 1e6 * self.px_per_sec >= MIN_LABEL_GAP)
            .unwrap_or(STEPS[STEPS.len() - 1]);
        TimeDelta::from_micros(micros)
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

/// The label of a ruler tick at `t` with ticks `step` apart: «1:10», «0:33.5», «0:33.25» — as
/// many digits after the point as the step needs.
pub fn tick_label(t: OutputTime, step: TimeDelta) -> String {
    let micros = t.micros().max(0);
    let seconds = ruler_label(micros / 1_000_000);
    let step = step.micros();
    if step % 1_000_000 == 0 {
        seconds
    } else if step % 100_000 == 0 {
        format!("{seconds}.{}", micros % 1_000_000 / 100_000)
    } else {
        format!("{seconds}.{:02}", micros % 1_000_000 / 10_000)
    }
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

    /// `width` points for `secs` seconds from x `left`.
    fn whole(left: f32, width: f32, secs: f64) -> TimeScale {
        TimeScale {
            left,
            start: OutputTime::ZERO,
            px_per_sec: width / secs as f32,
        }
    }

    #[test]
    fn time_maps_to_x_from_the_left_edge() {
        let scale = whole(148.0, 1000.0, 100.0);
        assert_eq!(scale.x(OutputTime::ZERO), 148.0);
        assert_eq!(scale.x(at(100.0)), 1148.0);
        assert_eq!(scale.x(at(33.4)), 482.0);
    }

    #[test]
    fn x_and_time_are_inverse() {
        let scale = whole(148.0, 900.0, 108.0);
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
    fn ticks_keep_labels_apart() {
        let step = |width: f32, secs: f64| whole(0.0, width, secs).tick_step().as_secs_f64();
        assert_eq!(step(1132.0, 108.0), 10.0);
        assert_eq!(step(1132.0, 20.0), 1.0);
        assert_eq!(step(1132.0, 10.0), 0.5);
        assert_eq!(step(1132.0, 3600.0), 300.0);
        assert_eq!(step(1500.0, 1.0), 0.05, "a few frames per 100 points");
        assert_eq!(step(1000.0, 4.0), 0.25);
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

    #[test]
    fn tick_labels_have_the_digits_of_their_step() {
        let label = |secs: f64, step: f64| tick_label(at(secs), TimeDelta::from_secs_f64(step));
        assert_eq!(label(70.0, 10.0), "1:10");
        assert_eq!(label(33.5, 0.5), "0:33.5");
        assert_eq!(label(33.25, 0.25), "0:33.25");
        assert_eq!(label(33.05, 0.05), "0:33.05");
        assert_eq!(label(33.1, 0.1), "0:33.1");
    }
}
