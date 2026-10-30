//! The sprite of an image annotation: the picture fitted into the box keeping its proportions
//! and centred (v1 «contain», `annotationRenderer.ts:599-615`), turned with the box.

use dc_core::TimeDelta;
use dc_fx_annotations::Placement;
use dc_render::{ImageKey, PxRect, Sprite};

use crate::model::ImageBody;

/// The sprite of `body` placed at `placement`, `elapsed` output time after the annotation first
/// shows (the clock of an animated picture); `None` without a picture.
pub fn evaluate(body: &ImageBody, placement: &Placement, elapsed: TimeDelta) -> Option<Sprite> {
    let picture = body.picture.as_ref()?;
    let [w, h] = picture.size.map(|v| v as f32);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = placement.rect;
    let scale = (r.w / w).min(r.h / h);
    let (fw, fh) = (w * scale, h * scale);
    Some(Sprite {
        image: ImageKey {
            path: picture.path.clone(),
            frame: picture.frame_at(elapsed),
        },
        rect: PxRect {
            x: r.x + (r.w - fw) / 2.0,
            y: r.y + (r.h - fh) / 2.0,
            w: fw,
            h: fh,
        },
        rotation: placement.rotation,
        opacity: placement.opacity,
        motion: None,
    })
}
