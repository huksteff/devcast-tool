//! Following the device clock: a PI loop turns the offset of packets from their place on the
//! host clock into a small speed correction of the resampler. The integral part learns the
//! drift itself, so a steady drift leaves no steady offset.

/// The offset is smoothed over about this long: packet times jitter a little.
const SMOOTH_SECONDS: f64 = 0.5;

/// Proportional part: an offset is pulled in over about this long.
const PULL_SECONDS: f64 = 2.0;

/// Integral part; with [`PULL_SECONDS`] the loop is critically damped and settles in ~15 s.
const LEARN_SECONDS: f64 = 8.0;

/// The largest speed correction: 1000 ppm, under 2 cents of pitch.
pub const MAX_CORRECTION: f64 = 0.001;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DriftLoop {
    smoothed: f64,
    integral: f64,
    correction: f64,
    /// The largest offset seen while following, seconds.
    worst: f64,
}

impl DriftLoop {
    /// A packet `offset` seconds ahead of its place (negative: late), carrying `seconds` of
    /// sound. Returns the relative speed for the resampler.
    pub fn update(&mut self, offset: f64, seconds: f64) -> f64 {
        self.worst = self.worst.max(offset.abs());
        self.smoothed += (offset - self.smoothed) * (seconds / SMOOTH_SECONDS).min(1.0);
        self.integral = (self.integral + self.smoothed * seconds / (PULL_SECONDS * LEARN_SECONDS))
            .clamp(-MAX_CORRECTION, MAX_CORRECTION);
        self.correction =
            (self.smoothed / PULL_SECONDS + self.integral).clamp(-MAX_CORRECTION, MAX_CORRECTION);
        1.0 - self.correction
    }

    /// Current speed correction, ppm.
    pub fn correction_ppm(&self) -> f64 {
        -self.correction * 1e6
    }

    pub fn worst(&self) -> f64 {
        self.worst
    }

    /// A new device: its clock is unknown; the worst offset is kept.
    pub fn restart(&mut self) {
        *self = Self {
            worst: self.worst,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds a device whose clock runs `drift` faster than the host for `seconds`, in 10 ms
    /// packets; returns the offset at the end, seconds.
    fn follow(drift: f64, seconds: f64, jitter: f64) -> (DriftLoop, f64) {
        let mut drift_loop = DriftLoop::default();
        let mut offset = 0.0;
        let packet = 0.01;
        let steps = (seconds / packet) as usize;
        for step in 0..steps {
            // Deterministic jitter: alternating ahead and late.
            let noise = if step % 2 == 0 { jitter } else { -jitter };
            let speed = drift_loop.update(offset + noise, packet);
            // The device delivers `packet × (1 + drift)` of sound per `packet` of host time.
            offset += packet * ((1.0 + drift) * speed - 1.0);
        }
        (drift_loop, offset)
    }

    #[test]
    fn steady_drift_is_learned_and_leaves_no_offset() {
        for drift in [41e-6, -41e-6, 500e-6] {
            let (drift_loop, offset) = follow(drift, 60.0, 0.0);
            assert!(offset.abs() < 1e-5, "{drift}: {offset}");
            let learned = drift_loop.correction_ppm();
            assert!((learned + drift * 1e6).abs() < 1.0, "{drift}: {learned}");
        }
    }

    #[test]
    fn jitter_barely_moves_the_speed() {
        let (drift_loop, offset) = follow(41e-6, 60.0, 0.001);
        assert!(offset.abs() < 1e-4, "{offset}");
        assert!((drift_loop.correction_ppm() + 41.0).abs() < 25.0);
        assert!(drift_loop.worst() >= 0.001);
    }

    #[test]
    fn correction_is_limited() {
        let (drift_loop, _) = follow(0.01, 10.0, 0.0);
        assert_eq!(drift_loop.correction_ppm(), -MAX_CORRECTION * 1e6);
    }
}
