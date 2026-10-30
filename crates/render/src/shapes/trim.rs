//! Draw-in of a figure (`Shape::drawn`): the length of its path that is drawn and where the drawn
//! part ends, in the figure's own axes, camera applied. The paths are the ones shapes.wgsl
//! measures a fragment along (`path_position`).

use std::f32::consts::{FRAC_PI_2, PI};

/// Kinds of shapes.wgsl.
pub(super) const RECT: f32 = 0.0;
pub(super) const ELLIPSE: f32 = 1.0;
pub(super) const LINE: f32 = 2.0;
pub(super) const ARROW: f32 = 3.0;

/// `trim` and `ends` rows of shapes.wgsl for a figure of `kind` with per-kind `lengths` (the
/// `geometry` row) drawn `drawn` of its path: `[drawn path px, drawn, 0, 0]` and, for a closed
/// figure, the two ends of the drawn part (head, start). A line or an arrow needs no ends: the
/// shader measures the distance to its drawn segments.
pub(super) fn trim(kind: f32, lengths: [f32; 3], drawn: f32) -> ([f32; 4], [f32; 4]) {
    if drawn >= 1.0 {
        return ([0.0, 1.0, 0.0, 0.0], [0.0; 4]);
    }
    let drawn = drawn.max(0.0);
    let [l0, l1, l2] = lengths;
    let (length, ends) = if kind == RECT {
        let length = rect_length(l0, l1, l2) * drawn;
        let [start, head] = [0.0, length].map(|s| rect_point(l0, l1, l2, s));
        (length, [head, start])
    } else if kind == ELLIPSE {
        let length = ellipse_length(l0, l1) * drawn;
        let [start, head] = [0.0, length].map(|s| ellipse_point(l0, l1, s));
        (length, [head, start])
    } else if kind == LINE {
        (2.0 * l0 * drawn, [[0.0; 2]; 2])
    } else {
        ((2.0 * l0 + l2) * drawn, [[0.0; 2]; 2])
    };
    let [[ax, ay], [bx, by]] = ends;
    ([length, drawn, 0.0, 0.0], [ax, ay, bx, by])
}

/// Corner radius as shapes.wgsl draws it: at most half the shorter side.
fn radius(hx: f32, hy: f32, r: f32) -> f32 {
    r.min(hx).min(hy).max(0.0)
}

fn rect_length(hx: f32, hy: f32, r: f32) -> f32 {
    let r = radius(hx, hy, r);
    4.0 * (hx - r) + 4.0 * (hy - r) + 2.0 * PI * r
}

/// The point `s` px along the outline of a rounded rectangle of half size `hx, hy`: from the
/// start of the top side (its top-left corner) clockwise on screen.
fn rect_point(hx: f32, hy: f32, r: f32, s: f32) -> [f32; 2] {
    let r = radius(hx, hy, r);
    let (ix, iy) = (hx - r, hy - r);
    let mut s = s.max(0.0);
    for piece in 0..8_u32 {
        let side = if piece.is_multiple_of(4) {
            2.0 * ix
        } else {
            2.0 * iy
        };
        let length = if piece.is_multiple_of(2) {
            side
        } else {
            r * FRAC_PI_2
        };
        if s <= length || piece == 7 {
            let t = s.min(length);
            let turn = if r > 0.0 { t / r } else { 0.0 };
            let (sin, cos) = turn.sin_cos();
            return match piece {
                0 => [-ix + t, -hy],
                1 => [ix + r * sin, -iy - r * cos],
                2 => [hx, -iy + t],
                3 => [ix + r * cos, iy + r * sin],
                4 => [ix - t, hy],
                5 => [-ix - r * sin, iy + r * cos],
                6 => [-hx, iy - t],
                _ => [-ix - r * cos, -iy - r * sin],
            };
        }
        s -= length;
    }
    [-ix, -hy]
}

/// Length of the ellipse arc with half axes `a`, `b` from the top to parametric angle `x`
/// (0..π/2): Simpson's rule on 8 intervals, as shapes.wgsl.
fn ellipse_arc(a: f32, b: f32, x: f32) -> f32 {
    let h = x / 8.0;
    let sum: f32 = (0..=8_u32)
        .map(|i| {
            let u = i as f32 * h;
            let weight = match i {
                0 | 8 => 1.0,
                i if !i.is_multiple_of(2) => 4.0,
                _ => 2.0,
            };
            weight * (a * u.cos()).hypot(b * u.sin())
        })
        .sum();
    sum * h / 3.0
}

fn ellipse_length(a: f32, b: f32) -> f32 {
    4.0 * ellipse_arc(a, b, FRAC_PI_2)
}

/// Arc length from the top clockwise to parametric angle `t` (0..2π), as shapes.wgsl.
fn ellipse_position(a: f32, b: f32, t: f32) -> f32 {
    let quarter = ellipse_arc(a, b, FRAC_PI_2);
    let q = (t / FRAC_PI_2).floor().clamp(0.0, 3.0);
    let tau = t - q * FRAC_PI_2;
    let within = if (q as u32).is_multiple_of(2) {
        ellipse_arc(a, b, tau)
    } else {
        quarter - ellipse_arc(a, b, FRAC_PI_2 - tau)
    };
    q * quarter + within
}

/// The point `s` px along the ellipse from the top clockwise on screen.
fn ellipse_point(a: f32, b: f32, s: f32) -> [f32; 2] {
    let (mut low, mut high) = (0.0_f32, 2.0 * PI);
    for _ in 0..32 {
        let mid = (low + high) / 2.0;
        if ellipse_position(a, b, mid) < s {
            low = mid;
        } else {
            high = mid;
        }
    }
    let (sin, cos) = ((low + high) / 2.0).sin_cos();
    [a * sin, -b * cos]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 2], b: [f32; 2]) {
        assert!(
            (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01,
            "{a:?} {b:?}"
        );
    }

    #[test]
    fn rectangle_path_starts_top_left_and_runs_clockwise() {
        // 100×60 with radius 10: sides 80 and 40, quarter arcs 5π.
        let (hx, hy, r) = (50.0, 30.0, 10.0);
        near(rect_point(hx, hy, r, 0.0), [-40.0, -30.0]);
        near(rect_point(hx, hy, r, 80.0), [40.0, -30.0]);
        near(rect_point(hx, hy, r, 80.0 + 5.0 * PI), [50.0, -20.0]);
        near(rect_point(hx, hy, r, 120.0 + 10.0 * PI), [40.0, 30.0]);
        let total = rect_length(hx, hy, r);
        assert!((total - (240.0 + 20.0 * PI)).abs() < 1e-3);
        near(rect_point(hx, hy, r, total), [-40.0, -30.0]);
        near(rect_point(50.0, 30.0, 0.0, 100.0 + 30.0), [50.0, 0.0]);
    }

    #[test]
    fn circle_path_is_exact_and_starts_at_the_top() {
        let total = ellipse_length(10.0, 10.0);
        assert!((total - 20.0 * PI).abs() < 1e-3, "{total}");
        near(ellipse_point(10.0, 10.0, 0.0), [0.0, -10.0]);
        near(ellipse_point(10.0, 10.0, total / 4.0), [10.0, 0.0]);
        near(ellipse_point(10.0, 10.0, total / 2.0), [0.0, 10.0]);
    }

    #[test]
    fn ellipse_length_matches_ramanujan() {
        let (a, b) = (100.0_f32, 40.0_f32);
        let h = ((a - b) / (a + b)).powi(2);
        let ramanujan = PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()));
        assert!(
            (ellipse_length(a, b) - ramanujan).abs() < 0.05,
            "{}",
            ellipse_length(a, b)
        );
        near(
            ellipse_point(a, b, ellipse_length(a, b) / 4.0),
            [100.0, 0.0],
        );
    }

    #[test]
    fn arrow_path_is_its_shaft_and_one_whisker() {
        let ([length, drawn, ..], _) = trim(ARROW, [50.0, 4.0, 20.0], 0.5);
        assert_eq!((length, drawn), (60.0, 0.5));
    }

    #[test]
    fn whole_figure_has_no_trim() {
        assert_eq!(trim(RECT, [10.0, 10.0, 0.0], 1.0).0[1], 1.0);
        assert_eq!(trim(LINE, [10.0, 2.0, 0.0], -1.0).0, [0.0; 4]);
    }
}
