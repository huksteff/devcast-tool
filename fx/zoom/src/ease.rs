//! The zoom curve: CSS `cubic-bezier(0.16, 1, 0.3, 1)`, v1 `easeOutZoom` (docs/porting/zoom.md
//! §4.2): a quick start and a long soft landing. The camera spring smooths it further.

const X1: f64 = 0.16;
const Y1: f64 = 1.0;
const X2: f64 = 0.3;
const Y2: f64 = 1.0;

/// Bisection steps on the curve parameter: 2⁻⁴⁰ is far below a pixel of any frame.
const STEPS: u32 = 40;

fn bezier(p1: f64, p2: f64, s: f64) -> f64 {
    let r = 1.0 - s;
    3.0 * p1 * r * r * s + 3.0 * p2 * r * s * s + s * s * s
}

/// Eased progress, 0..1, of linear progress `p` (clamped to 0..1).
pub(crate) fn ease(p: f64) -> f64 {
    let x = p.clamp(0.0, 1.0);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..STEPS {
        let mid = (lo + hi) / 2.0;
        if bezier(X1, X2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bezier(Y1, Y2, (lo + hi) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::ease;

    #[test]
    fn matches_the_v1_control_values() {
        let expected = [
            (0.0, 0.0),
            (0.05, 0.28106),
            (0.1, 0.49439),
            (0.2, 0.75213),
            (0.3, 0.87717),
            (0.5, 0.97178),
            (0.7, 0.99568),
            (0.9, 0.99988),
            (1.0, 1.0),
        ];
        for (p, e) in expected {
            assert!((ease(p) - e).abs() < 1e-5, "E({p}) = {} ≠ {e}", ease(p));
        }
        assert!(ease(-1.0).abs() < 1e-9);
        assert!((ease(2.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn never_goes_back() {
        let values: Vec<f64> = (0..=1000).map(|i| ease(f64::from(i) / 1000.0)).collect();
        assert!(values.windows(2).all(|w| w[0] <= w[1]));
    }
}
