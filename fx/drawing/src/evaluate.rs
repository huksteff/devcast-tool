//! The drawing of an annotation in the scene (docs/porting/annotations.md §8): points of the box
//! in canvas px turned with the box, thickness in reference px whatever the box (decision 000),
//! draw-in through `Drawing::drawn` (the brush pass measures the path in px, not in fractions of
//! the box as v1, so a long box does not change the speed).

use dc_fx_annotations::Placement;
use dc_render::{BrushStroke, Drawing, Tool};

use crate::model::{DrawingBody, StrokeTool};

/// Thinnest stroke on the canvas, px (v1 `max(1, strokeWidth·k)`).
const MIN_PX: f64 = 1.0;

/// The drawing of `body` placed at `placement`, for `Band::Annotations` under the camera; `None`
/// while it has no strokes.
pub fn evaluate(body: &DrawingBody, placement: &Placement) -> Option<Drawing> {
    if body.strokes.is_empty() {
        return None;
    }
    let r = placement.rect;
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let (sin, cos) = placement.rotation.to_radians().sin_cos();
    let point = |[x, y]: [f64; 2]| {
        let dx = r.x + x as f32 * r.w - cx;
        let dy = r.y + y as f32 * r.h - cy;
        [cx + dx * cos - dy * sin, cy + dx * sin + dy * cos]
    };
    let strokes = body
        .strokes
        .iter()
        .map(|stroke| BrushStroke {
            points: stroke.points.iter().copied().map(point).collect(),
            width: (stroke.width * placement.k).max(MIN_PX) as f32,
            tool: match stroke.tool {
                StrokeTool::Pencil => Tool::Pen(stroke.color),
                StrokeTool::Eraser => Tool::Eraser,
            },
        })
        .collect();
    Some(Drawing {
        strokes,
        drawn: placement.reveal as f32,
        opacity: (body.opacity * f64::from(placement.opacity)) as f32,
    })
}
