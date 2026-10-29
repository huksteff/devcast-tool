//! The camera spring (docs/porting/zoom.md §5): v1 at smoothness 0.5 has k = 100, c = 21·1.13,
//! m = 1.12 — slightly overdamped. The smoothness slider (decision 000) scales its time: the
//! damping ratio stays, the natural frequency is `OMEGA / (2·smoothness)`; at 0 the camera sits
//! on its target.

const STIFFNESS: f64 = 100.0;
const DAMPING: f64 = 21.0 * 1.13;
const MASS: f64 = 1.12;

/// One axis of the camera: how far it is from its target and how fast it moves, units per second.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Axis {
    pub offset: f64,
    pub speed: f64,
}

/// An overdamped spring stepped by a fixed time: the exact solution of `m·x″ + c·x′ + k·x = 0`
/// over the step, the target constant within it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spring {
    /// Decay rates of the two modes, 1/s, `slow < fast`.
    slow: f64,
    fast: f64,
    /// `e^(−rate·step)` of both modes.
    slow_step: f64,
    fast_step: f64,
}

impl Spring {
    /// The spring of `smoothness` (0..1) stepped by `step` seconds; `None` at 0: no spring.
    pub(crate) fn new(smoothness: f64, step: f64) -> Option<Self> {
        if smoothness <= 0.0 {
            return None;
        }
        let omega = (STIFFNESS / MASS).sqrt() / (2.0 * smoothness);
        let zeta = DAMPING / (2.0 * (STIFFNESS * MASS).sqrt());
        let spread = (zeta * zeta - 1.0).sqrt();
        let (slow, fast) = (omega * (zeta - spread), omega * (zeta + spread));
        Some(Self {
            slow,
            fast,
            slow_step: (-slow * step).exp(),
            fast_step: (-fast * step).exp(),
        })
    }

    /// Seconds after which a jump of the target from rest is at least 99 % done: the offset is
    /// `(fast·e^(−slow·t) − slow·e^(−fast·t)) / (fast − slow)` of the jump.
    pub(crate) fn settle(&self) -> f64 {
        (100.0 * self.fast / (self.fast - self.slow)).ln() / self.slow
    }

    /// The axis one step later.
    pub(crate) fn step(&self, axis: Axis) -> Axis {
        let slow = (axis.speed + self.fast * axis.offset) / (self.fast - self.slow);
        let fast = axis.offset - slow;
        let (slow, fast) = (slow * self.slow_step, fast * self.fast_step);
        Axis {
            offset: slow + fast,
            speed: -self.slow * slow - self.fast * fast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Axis, Spring};

    const STEP: f64 = 1.0 / 240.0;

    fn run(spring: &Spring, seconds: f64) -> Axis {
        let steps = (seconds / STEP).round() as u32;
        (0..steps).fold(
            Axis {
                offset: 1.0,
                speed: 0.0,
            },
            |axis, _| spring.step(axis),
        )
    }

    #[test]
    fn v1_spring_reaches_the_target_without_overshoot_in_under_0_9_s() {
        let spring = Spring::new(0.5, STEP).unwrap();
        assert!(
            (spring.settle() - 0.875).abs() < 0.01,
            "{}",
            spring.settle()
        );
        let mut axis = Axis {
            offset: 1.0,
            speed: 0.0,
        };
        for _ in 0..480 {
            let next = spring.step(axis);
            assert!(next.offset < axis.offset && next.offset > 0.0);
            axis = next;
        }
        assert!(run(&spring, 0.172).offset > 0.4);
        assert!(run(&spring, spring.settle() + STEP).offset <= 0.01);
        assert!(run(&spring, spring.settle() - 0.05).offset > 0.01);
    }

    #[test]
    fn smoothness_scales_the_time() {
        let soft = Spring::new(1.0, STEP).unwrap();
        let sharp = Spring::new(0.25, STEP).unwrap();
        assert!((soft.settle() / sharp.settle() - 4.0).abs() < 1e-9);
        assert!((run(&soft, 0.8).offset - run(&sharp, 0.2).offset).abs() < 1e-3);
        assert_eq!(Spring::new(0.0, STEP), None);
    }

    #[test]
    fn step_matches_the_formula_of_motion() {
        let spring = Spring::new(0.5, STEP).unwrap();
        let axis = Axis {
            offset: 0.3,
            speed: -2.0,
        };
        let next = spring.step(axis);
        let (k, c, m) = (100.0, 21.0 * 1.13, 1.12);
        let accel = -(k * axis.offset + c * axis.speed) / m;
        let predicted = axis.speed + accel * STEP;
        assert!(
            (next.speed - predicted).abs() < 0.2,
            "{next:?} vs {predicted}"
        );
        assert!((next.offset - (axis.offset + axis.speed * STEP)).abs() < 1e-3);
    }
}
