//! The figure of a shape annotation in the scene (docs/porting/annotations.md §6): thickness in
//! reference px whatever the box (decision 000), the outline inside the box, arrow and line
//! between two points of the box turned with it; draw-in through `Shape::drawn`.

use dc_core::OutputTime;
use dc_fx_annotations::Placement;
use dc_render::{Geometry, Glow, Gradient, Paint, PxRect, Rgba, Shape, Stroke};

use crate::model::{BoxPoint, Figure, ShapeBody};

/// Whisker of an arrow per its length and per its thickness: the v1 arrow (whisker 15√2 on a
/// 60-unit shaft, 4 units thick), whichever is shorter, so a long arrow keeps its head and a
/// short one is not all head.
const HEAD_PER_LENGTH: f64 = 0.354;
const HEAD_PER_WIDTH: f64 = 5.3;
/// Glow σ per glow unit and ref-px (v1 figures: `shadowBlur = glow · 1.5` = 2σ).
const GLOW_SIGMA: f64 = 0.75;

/// The figure of `body` placed at `placement` at output moment `t` (the gradient flows by the
/// output video), for `Band::Annotations` under the camera.
pub fn evaluate(body: &ShapeBody, placement: &Placement, t: OutputTime) -> Shape {
    let width = body.width * placement.k;
    let open = body.figure.is_open();
    let paint = |solid: Rgba| match body.gradient {
        true => Some(gradient(body, t)),
        false => (solid.0[3] > 0).then_some(Paint::Solid(solid)),
    };
    let (geometry, rotation, fill, stroke) = if open {
        (
            segment(body, placement, width),
            0.0,
            paint(body.color),
            None,
        )
    } else {
        let stroke = body.outline.then_some(Stroke {
            width: width as f32,
            color: body.color,
        });
        let inset = if body.outline { width / 2.0 } else { 0.0 };
        let geometry = closed(body, placement, inset);
        (geometry, placement.rotation, paint(body.fill), stroke)
    };
    Shape {
        geometry,
        fill,
        stroke,
        opacity: (body.opacity * f64::from(placement.opacity)) as f32,
        rotation,
        glow: glow(body, placement.k),
        drawn: placement.reveal as f32,
    }
}

/// Arrow or line between its points of the box, turned with the box around its centre.
fn segment(body: &ShapeBody, placement: &Placement, width: f64) -> Geometry {
    let r = placement.rect;
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let (sin, cos) = placement.rotation.to_radians().sin_cos();
    let point = |p: BoxPoint| {
        let dx = r.x + p.x as f32 * r.w - cx;
        let dy = r.y + p.y as f32 * r.h - cy;
        [cx + dx * cos - dy * sin, cy + dx * sin + dy * cos]
    };
    let (from, to) = (point(body.from), point(body.to));
    let width = width as f32;
    if body.figure == Figure::Line {
        return Geometry::Line { from, to, width };
    }
    let length = f64::from((to[0] - from[0]).hypot(to[1] - from[1]));
    let head = (HEAD_PER_LENGTH * length).min(HEAD_PER_WIDTH * f64::from(width));
    Geometry::Arrow {
        from,
        to,
        width,
        head: head as f32,
    }
}

/// Rectangle, ellipse, circle or highlight in the box, `inset` px in so the outline stays inside.
fn closed(body: &ShapeBody, placement: &Placement, inset: f64) -> Geometry {
    let r = placement.rect;
    let (w, h) = match body.figure {
        Figure::Circle => (r.w.min(r.h), r.w.min(r.h)),
        _ => (r.w, r.h),
    };
    let inset = inset as f32;
    let rect = PxRect {
        x: r.x + (r.w - w) / 2.0 + inset,
        y: r.y + (r.h - h) / 2.0 + inset,
        w: (w - 2.0 * inset).max(0.0),
        h: (h - 2.0 * inset).max(0.0),
    };
    match body.figure {
        Figure::Ellipse | Figure::Circle => Geometry::Ellipse { rect },
        _ => Geometry::Rect {
            rect,
            radius: (body.radius * placement.k) as f32,
        },
    }
}

/// Still or flowing in a seamless loop of `cycle` seconds of the output video, so a sped-up
/// part does not hurry it (decision 000).
fn gradient(body: &ShapeBody, t: OutputTime) -> Paint {
    let secs = t.since(OutputTime::ZERO).as_secs_f64();
    Paint::Gradient(Gradient {
        colors: body.colors.clone(),
        angle: body.angle as f32,
        cycle: (body.cycle > 0.0).then(|| (secs / body.cycle).fract() as f32),
    })
}

/// Real glow with or without a gradient (decision 000): the gradient's first colour, else what
/// paints the figure (the fill if there is one, else the line or outline colour).
fn glow(body: &ShapeBody, k: f64) -> Option<Glow> {
    if body.glow <= 0.0 {
        return None;
    }
    let color = if body.gradient {
        body.colors.colors()[0]
    } else if !body.figure.is_open() && body.fill.0[3] > 0 {
        body.fill
    } else {
        body.color
    };
    Some(Glow {
        color,
        sigma: (GLOW_SIGMA * body.glow * k) as f32,
    })
}
