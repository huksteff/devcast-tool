//! The kind's picture in the frame (`dc_fx_annotations::look`): its fields as a layer.

use dc_fx_annotations::Placement;
use dc_fx_annotations::look::{Body, Moment, Read, read};
use dc_render::Layer;

use crate::BlurBody;

/// How the annotations' picture reads this kind (the registration's table of kinds).
pub const READ: Read = read::<BlurBody>;

impl Body for BlurBody {
    fn layer(&self, placement: &Placement, _: &Moment<'_>) -> Option<Layer> {
        Some(Layer::Blur(crate::evaluate(self, placement)))
    }
}
