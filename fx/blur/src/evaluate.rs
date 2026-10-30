//! The region of a blur annotation in the scene: what lies below its box is blurred there
//! (docs/porting/annotations.md §9). The region is blurred where it stands, so a pop or a slide
//! moves the blurred place with the box (v1 blurred the place before the move).

use dc_fx_annotations::Placement;
use dc_render::Blur;

use crate::model::BlurBody;

/// The blur of `body` placed at `placement`, for `Band::Annotations` under the camera: σ and the
/// corner radius are reference px (a pop scales them with the box), the turn and the opacity of
/// the entrance or exit are the annotation's.
pub fn evaluate(body: &BlurBody, placement: &Placement) -> Blur {
    Blur {
        rect: placement.rect,
        radius: (body.radius * placement.k) as f32,
        sigma: (body.strength * placement.k) as f32,
        tint: body.tint,
        rotation: placement.rotation,
        opacity: placement.opacity,
    }
}
