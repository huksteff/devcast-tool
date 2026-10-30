//! Where an annotation stands in a frame: the part every kind draws from. Entrance and exit
//! animations (5.30) and keyframes (5.31) change this, not the kinds.

use dc_core::PixelSize;
use dc_render::PxRect;

use crate::model::Annotation;

/// Lengths of the kinds (font size, plate radius, stroke) are px when the short side of the
/// frame is this many px, as captions and keys.
pub const REFERENCE: f64 = 1080.0;

/// An annotation in the canvas of one frame, before the zoom camera (decision 000): the scene
/// camera moves and scales it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Box, px of the canvas.
    pub rect: PxRect,
    /// Degrees clockwise around the box centre.
    pub rotation: f32,
    /// Canvas px per reference px ([`REFERENCE`]).
    pub k: f64,
}

impl Placement {
    pub fn of(annotation: &Annotation, canvas: PixelSize) -> Self {
        let (w, h) = (f64::from(canvas.width), f64::from(canvas.height));
        let r = annotation.rect;
        Self {
            rect: PxRect {
                x: (r.x() * w) as f32,
                y: (r.y() * h) as f32,
                w: (r.w() * w) as f32,
                h: (r.h() * h) as f32,
            },
            rotation: annotation.rotation as f32,
            k: w.min(h) / REFERENCE,
        }
    }
}
