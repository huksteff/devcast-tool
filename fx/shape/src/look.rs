//! The kind's picture in the frame (`dc_fx_annotations::look`): its fields as a layer.

use dc_fx_annotations::Placement;
use dc_fx_annotations::look::{Body, Moment, Read, read};
use dc_render::Layer;

use crate::ShapeBody;

/// How the annotations' picture reads this kind (the registration's table of kinds).
pub const READ: Read = read::<ShapeBody>;

impl Body for ShapeBody {
    fn layer(&self, placement: &Placement, moment: &Moment<'_>) -> Option<Layer> {
        Some(Layer::Shapes(vec![crate::evaluate(
            self, placement, moment.t,
        )]))
    }
}
