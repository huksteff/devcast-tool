//! The kind's picture in the frame (`dc_fx_annotations::look`): its fields as a layer.

use dc_fx_annotations::Placement;
use dc_fx_annotations::look::{Body, Moment, Read, read};
use dc_render::Layer;

use crate::DrawingBody;

/// How the annotations' picture reads this kind (the registration's table of kinds).
pub const READ: Read = read::<DrawingBody>;

impl Body for DrawingBody {
    fn layer(&self, placement: &Placement, _: &Moment<'_>) -> Option<Layer> {
        crate::evaluate(self, placement).map(Layer::Drawing)
    }
}
