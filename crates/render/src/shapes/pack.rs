//! Figures as the `Shape` struct of shapes.wgsl, camera applied.

use crate::scene::{Camera, Geometry, Paint, Shape};

/// Floats of one figure in the storage buffer: `Shape` of shapes.wgsl, 11 × vec4.
pub(super) const FLOATS: usize = 44;

/// Kind, centre, half size of the bounds, per-kind lengths and own angle (radians) of a figure.
struct Placed {
    kind: f32,
    center: [f32; 2],
    half: [f32; 2],
    lengths: [f32; 3],
    angle: f32,
}

fn place(geometry: &Geometry) -> Placed {
    let box_of = |kind, r: &crate::scene::PxRect, radius| Placed {
        kind,
        center: [r.x + r.w / 2.0, r.y + r.h / 2.0],
        half: [r.w / 2.0, r.h / 2.0],
        lengths: [r.w / 2.0, r.h / 2.0, radius],
        angle: 0.0,
    };
    let segment = |kind, from: [f32; 2], to: [f32; 2], width: f32, head: f32| {
        let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
        let half = dx.hypot(dy) / 2.0;
        let back = head * std::f32::consts::FRAC_1_SQRT_2;
        Placed {
            kind,
            center: [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0],
            half: [half.max(back - half) + width / 2.0, back + width / 2.0],
            lengths: [half, width / 2.0, head],
            angle: dy.atan2(dx),
        }
    };
    match *geometry {
        Geometry::Rect { ref rect, radius } => box_of(0.0, rect, radius),
        Geometry::Ellipse { ref rect } => box_of(1.0, rect, 0.0),
        Geometry::Line { from, to, width } => segment(2.0, from, to, width, 0.0),
        Geometry::Arrow {
            from,
            to,
            width,
            head,
        } => segment(3.0, from, to, width, head),
    }
}

/// The `Shape` struct of shapes.wgsl for `shape` seen through `camera`.
pub(super) fn pack(shape: &Shape, camera: Camera) -> [f32; FLOATS] {
    let p = place(&shape.geometry);
    let k = camera.scale;
    let [cx, cy] = camera.apply(p.center);
    let (sin, cos) = (p.angle + shape.rotation.to_radians()).sin_cos();
    let stroke_width = shape.stroke.as_ref().map_or(0.0, |s| s.width * k);
    let mut colors = [[0.0; 4]; 4];
    let (count, cycle, phase, angle) = match &shape.fill {
        None => (0, None, 0.0, 0.0),
        Some(Paint::Solid(color)) => {
            colors[0] = unit(color.0);
            (1, None, 0.0, 0.0)
        }
        Some(Paint::Gradient(g)) => {
            for (slot, color) in colors.iter_mut().zip(g.colors.colors()) {
                *slot = unit(color.0);
            }
            (
                g.colors.colors().len(),
                g.cycle,
                g.cycle.unwrap_or(0.0),
                g.angle,
            )
        }
    };
    let (dir_y, dir_x) = angle.to_radians().sin_cos();
    let stroke = shape.stroke.as_ref().map_or([0.0; 4], |s| unit(s.color.0));
    let [l0, l1, l2] = p.lengths.map(|l| l * k);
    let head = [cx, cy, p.half[0] * k, p.half[1] * k];
    let transform = [cos, sin, stroke_width, shape.opacity];
    let style = [
        p.kind,
        count as f32,
        f32::from(u8::from(cycle.is_some())),
        phase,
    ];
    let geometry = [l0, l1, l2, 0.0];
    let direction = [dir_x, dir_y, 0.0, 0.0];
    let glow = shape.glow.as_ref().map_or([0.0; 4], |g| unit(g.color.0));
    let rows = [
        head, transform, style, geometry, direction, colors[0], colors[1], colors[2], colors[3],
        stroke, glow,
    ];
    let mut out = [0.0; FLOATS];
    out.copy_from_slice(rows.as_flattened());
    out
}

/// Frame px around a packed figure that its glow of `sigma` (frame px) reaches: the rotated
/// bounds with the outline, plus three sigmas. Left, top, right, bottom.
pub(super) fn glow_bounds(packed: &[f32; FLOATS], sigma: f32) -> [f32; 4] {
    let [cx, cy, hx, hy, cos, sin, stroke] = [0, 1, 2, 3, 4, 5, 6].map(|i| packed[i]);
    let pad = stroke / 2.0 + 3.0 * sigma;
    let ex = cos.abs() * hx + sin.abs() * hy + pad;
    let ey = sin.abs() * hx + cos.abs() * hy + pad;
    [cx - ex, cy - ey, cx + ex, cy + ey]
}

fn unit(rgba: [u8; 4]) -> [f32; 4] {
    rgba.map(|c| f32::from(c) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{PxRect, Rgba, Stroke};

    fn shape(geometry: Geometry) -> Shape {
        Shape {
            geometry,
            fill: Some(Paint::Solid(Rgba([255, 0, 0, 255]))),
            stroke: Some(Stroke {
                width: 4.0,
                color: Rgba([0, 0, 0, 255]),
            }),
            opacity: 1.0,
            rotation: 0.0,
            glow: None,
        }
    }

    #[test]
    fn camera_moves_centre_and_scales_every_length() {
        let rect = PxRect {
            x: 100.0,
            y: 50.0,
            w: 200.0,
            h: 100.0,
        };
        let packed = pack(
            &shape(Geometry::Rect { rect, radius: 10.0 }),
            Camera {
                scale: 2.0,
                x: -10.0,
                y: 0.0,
            },
        );
        assert_eq!(&packed[..4], &[390.0, 200.0, 200.0, 100.0], "centre, half");
        assert_eq!(packed[6], 8.0, "outline width");
        assert_eq!(&packed[12..15], &[200.0, 100.0, 20.0], "half size, radius");
    }

    #[test]
    fn arrow_bounds_hold_the_whiskers() {
        let arrow = Geometry::Arrow {
            from: [0.0, 0.0],
            to: [0.0, 100.0],
            width: 10.0,
            head: 200.0,
        };
        let packed = pack(&shape(arrow), Camera::IDENTITY);
        let back = 200.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(&packed[..2], &[0.0, 50.0]);
        assert!(
            (packed[2] - (back - 50.0 + 5.0)).abs() < 1e-3,
            "{}",
            packed[2]
        );
        assert!((packed[3] - (back + 5.0)).abs() < 1e-3, "{}", packed[3]);
        assert!(
            packed[4].abs() < 1e-6 && (packed[5] - 1.0).abs() < 1e-6,
            "points down"
        );
    }
}
