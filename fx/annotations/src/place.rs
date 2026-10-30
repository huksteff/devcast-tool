//! Where an annotation stands in a frame: the part every kind draws from. Entrance and exit
//! animations and keyframes change this, not the kinds.

use dc_core::{OutputTime, PixelSize, Rect, SourceTime, TimeMap};
use dc_render::PxRect;

use crate::model::Annotation;
use crate::motion::Pose;

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
    /// Canvas px per reference px ([`REFERENCE`]); a pop scales it with the box.
    pub k: f64,
    /// 0..1 for the whole annotation.
    pub opacity: f32,
    /// Part of the kind shown, 0..1: letters of a text (typewriter), path length of a shape or
    /// a drawing (draw-in).
    pub reveal: f64,
}

impl Placement {
    /// The annotation as it stands, without keys and animations.
    pub fn of(annotation: &Annotation, canvas: PixelSize) -> Self {
        Self::framed(annotation.rect, annotation.rotation, canvas)
    }

    /// Box `r` in fractions of the frame and `rotation` in degrees, in the canvas.
    fn framed(r: Rect, rotation: f64, canvas: PixelSize) -> Self {
        let (w, h) = (f64::from(canvas.width), f64::from(canvas.height));
        Self {
            rect: PxRect {
                x: (r.x() * w) as f32,
                y: (r.y() * h) as f32,
                w: (r.w() * w) as f32,
                h: (r.h() * h) as f32,
            },
            rotation: rotation as f32,
            k: w.min(h) / REFERENCE,
            opacity: 1.0,
            reveal: 1.0,
        }
    }

    /// The annotation at output moment `t`, which shows its recording at `now`: box and rotation
    /// from its keys, then its entrance or exit applied.
    pub fn at(
        annotation: &Annotation,
        canvas: PixelSize,
        map: &TimeMap,
        now: SourceTime,
        t: OutputTime,
    ) -> Self {
        let (rect, rotation) = annotation.frame_at(annotation.offset(now));
        Self::framed(rect, rotation, canvas).posed(&Pose::at(annotation, map, now, t))
    }

    /// Scaled around the box centre, then shifted (the shift is not scaled, as v1).
    pub fn posed(self, pose: &Pose) -> Self {
        let s = pose.scale as f32;
        let [dx, dy] = pose.shift.map(|d| (d * self.k) as f32);
        let r = self.rect;
        Self {
            rect: PxRect {
                x: r.x + r.w * (1.0 - s) / 2.0 + dx,
                y: r.y + r.h * (1.0 - s) / 2.0 + dy,
                w: r.w * s,
                h: r.h * s,
            },
            rotation: self.rotation,
            k: self.k * pose.scale,
            opacity: self.opacity * pose.opacity as f32,
            reveal: self.reveal * pose.reveal,
        }
    }
}
