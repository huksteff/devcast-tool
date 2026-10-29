//! How strongly one zoom pulls the camera at an output moment (docs/porting/zoom.md §4.1): 0 — not
//! at all, 1 — fully zoomed in. All times are seconds of the output video.

use crate::ease::ease;

/// The zoom-in starts this long before the zoom (v1: lead 200 ms, overlap 1000 ms, window
/// 1522.575 ms), whatever its duration.
const ENTER_LEAD: f64 = 0.322_575;
/// The zoom-out starts this long before the end of the zoom (v1: 500 ms early, 200 ms lead).
const EXIT_LEAD: f64 = 0.3;

/// The strength curve of one zoom: rises from `in0` to `in1`, holds `peak` up to `out0`, falls to
/// 0 at `out0 + exit`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ramp {
    in0: f64,
    in1: f64,
    out0: f64,
    enter: f64,
    exit: f64,
    peak: f64,
}

impl Ramp {
    /// A zoom shown over `start..end` with zoom-in and zoom-out durations `enter` and `exit`
    /// (positive). The zoom-out is moved earlier if needed to be over by `limit`. A zoom too
    /// short for both ramps meets them halfway and peaks below 1 (v1 jumped to 1 there).
    pub(crate) fn new(start: f64, end: f64, enter: f64, exit: f64, limit: f64) -> Self {
        let in0 = start - ENTER_LEAD;
        let mut in1 = in0 + enter;
        let mut out0 = end - EXIT_LEAD;
        if in1 > out0 {
            let middle = (in1 + out0) / 2.0;
            (in1, out0) = (middle, middle);
        }
        out0 = out0.min(limit - exit);
        in1 = in1.min(out0);
        Self {
            in0,
            in1,
            out0,
            enter,
            exit,
            peak: ease((in1 - in0) / enter),
        }
    }

    /// From the first moment the zoom pulls the camera to the moment it lets go.
    pub(crate) fn window(&self) -> (f64, f64) {
        (self.in0, self.out0 + self.exit)
    }

    pub(crate) fn strength(&self, t: f64) -> f64 {
        if t < self.in0 || t >= self.out0 + self.exit {
            0.0
        } else if t < self.in1 {
            ease((t - self.in0) / self.enter)
        } else if t <= self.out0 {
            self.peak
        } else {
            self.peak * (1.0 - ease((t - self.out0) / self.exit))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Ramp;

    const FOCUSED: f64 = 0.2;
    const FAR: f64 = 1000.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn focused_zoom_is_in_before_its_start_and_out_before_its_end() {
        let ramp = Ramp::new(2.0, 5.0, FOCUSED, FOCUSED, FAR);
        assert_eq!(ramp.window().0, 2.0 - 0.322_575);
        assert!(close(ramp.window().1, 4.9));
        assert_eq!(ramp.strength(1.677), 0.0);
        assert!(ramp.strength(1.8) > 0.5 && ramp.strength(1.8) < 1.0);
        assert!(close(ramp.strength(2.0 - 0.122_575), 1.0));
        assert!(close(ramp.strength(4.7), 1.0));
        assert!(ramp.strength(4.8) > 0.0 && ramp.strength(4.8) < 0.5);
        assert_eq!(ramp.strength(4.91), 0.0);
    }

    #[test]
    fn short_zoom_peaks_below_full_without_a_jump() {
        let smooth = Ramp::new(2.0, 3.0, 1.522_575, 1.015_05, FAR);
        let values: Vec<f64> = (0..400)
            .map(|i| smooth.strength(1.5 + f64::from(i) * 0.01))
            .collect();
        let peak = values.iter().copied().fold(0.0, f64::max);
        assert!(peak > 0.9 && peak < 1.0, "{peak}");
        assert!(values.windows(2).all(|w| (w[1] - w[0]).abs() < 0.1));
    }

    #[test]
    fn zoom_out_is_over_by_the_limit() {
        let ramp = Ramp::new(2.0, 10.0, FOCUSED, 1.0, 8.0);
        assert_eq!(ramp.window().1, 8.0);
        assert!(close(ramp.strength(6.9), 1.0));
        assert!(ramp.strength(7.5) < 1.0);
        let gone = Ramp::new(2.0, 3.0, FOCUSED, FOCUSED, 1.0);
        assert!((0..300).all(|i| gone.strength(f64::from(i) * 0.01) == 0.0));
    }
}
