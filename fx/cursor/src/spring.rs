//! The damped spring of v1 (docs/porting/cursor.md §5.1–5.2), solved in closed form over each
//! step: the step size barely changes the path.

use crate::state::Smoothing;

/// Mass-spring-damper; at rest when both the distance to the target and the speed are small.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spring {
    pub k: f64,
    pub c: f64,
    pub m: f64,
    /// Units of the value.
    pub rest_delta: f64,
    /// Units of the value per second.
    pub rest_speed: f64,
}

/// One axis following its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Axis {
    pub x: f64,
    /// Per second.
    pub v: f64,
}

impl Axis {
    pub fn at(x: f64) -> Self {
        Self { x, v: 0.0 }
    }
}

/// The spring of v1 `smoothing` in (0, 2] with multipliers `[k, c, m]` (§5.1).
pub(crate) fn config(smoothing: f64, [mul_k, mul_c, mul_m]: [f64; 3]) -> Spring {
    let (k, c, m) = if smoothing <= 0.5 {
        let n = smoothing / 0.5;
        ((760.0 - 420.0 * n) * 1.12, 34.0 + 24.0 * n, 0.85 + 0.55 * n)
    } else {
        let n = (smoothing - 0.5) / 1.5;
        ((340.0 - 180.0 * n) * 1.12, 58.0 + 22.0 * n, 1.35 + 0.45 * n)
    };
    Spring {
        k: k * mul_k,
        c: c * mul_c,
        m: m * mul_m,
        rest_delta: 0.0002,
        rest_speed: 0.01,
    }
}

impl Smoothing {
    /// The spring of the motion preset: v1 `smoothing` 0.67 and the preset's multipliers (§5.4).
    pub(crate) fn spring(self) -> Option<Spring> {
        match self {
            Self::Exact => None,
            Self::Focused => Some(config(0.67, [1.35, 0.79, 1.29])),
            Self::Smooth => Some(config(0.67, [0.92, 1.36, 1.29])),
        }
    }
}

impl Spring {
    /// Moves `axis` towards `target` for `dt` seconds (§5.2).
    pub fn step(&self, axis: &mut Axis, target: f64, dt: f64) {
        if (target - axis.x).abs() <= self.rest_delta && axis.v.abs() <= self.rest_speed {
            *axis = Axis::at(target);
            return;
        }
        let w0 = (self.k / self.m).sqrt();
        let z = self.c / (2.0 * (self.k * self.m).sqrt());
        let d = target - axis.x;
        let e1 = -axis.v;
        let error = |tau: f64| {
            if z < 1.0 {
                let wd = w0 * (1.0 - z * z).sqrt();
                (-z * w0 * tau).exp()
                    * ((e1 + z * w0 * d) / wd * (wd * tau).sin() + d * (wd * tau).cos())
            } else if z == 1.0 {
                (-w0 * tau).exp() * (d + (e1 + w0 * d) * tau)
            } else {
                let wd = w0 * (z * z - 1.0).sqrt();
                let a = (wd * tau).min(300.0);
                (-z * w0 * tau).exp() * ((e1 + z * w0 * d) * a.sinh() + wd * d * a.cosh()) / wd
            }
        };
        let now = target - error(dt);
        let overshot = (axis.x <= target && now > target) || (axis.x >= target && now < target);
        if z >= 1.0 && overshot {
            *axis = Axis::at(target);
            return;
        }
        let v = ((target - error(dt + 1e-4)) - now) / 1e-4;
        if v.abs() <= self.rest_speed && (target - now).abs() <= self.rest_delta {
            *axis = Axis::at(target);
        } else {
            *axis = Axis { x: now, v };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn presets_match_the_v1_table() {
        // docs/porting/cursor.md §5.4.
        let focused = config(0.67, [1.35, 0.79, 1.29]);
        assert!(close(focused.k, 483.2352) && close(focused.c, 47.7897));
        assert!(close(focused.m, 1.8073));
        let smooth = config(0.67, [0.92, 1.36, 1.29]);
        assert!(close(smooth.k, 329.3158) && close(smooth.c, 82.2709));
    }

    #[test]
    fn underdamped_spring_overshoots_then_settles() {
        let spring = config(0.67, [1.35, 0.79, 1.29]);
        let mut axis = Axis::at(0.0);
        let mut peak: f64 = 0.0;
        for _ in 0..480 {
            spring.step(&mut axis, 1.0, 1.0 / 240.0);
            peak = peak.max(axis.x);
        }
        assert!(peak > 1.0 && peak < 1.1, "{peak}");
        assert_eq!(axis, Axis::at(1.0));
    }

    #[test]
    fn overdamped_spring_never_passes_the_target() {
        let spring = config(0.67, [0.92, 1.36, 1.29]);
        let mut axis = Axis::at(0.0);
        for _ in 0..480 {
            spring.step(&mut axis, 1.0, 1.0 / 240.0);
            assert!(axis.x <= 1.0);
        }
        assert_eq!(axis, Axis::at(1.0));
    }

    #[test]
    fn step_size_barely_changes_the_path() {
        let spring = config(0.67, [1.35, 0.79, 1.29]);
        let (mut fine, mut coarse) = (Axis::at(0.0), Axis::at(0.0));
        for _ in 0..24 {
            spring.step(&mut fine, 1.0, 1.0 / 240.0);
        }
        for _ in 0..4 {
            spring.step(&mut coarse, 1.0, 1.0 / 40.0);
        }
        // Only the speed is a finite difference (v1).
        assert!((fine.x - coarse.x).abs() < 5e-3, "{fine:?} {coarse:?}");
    }
}
