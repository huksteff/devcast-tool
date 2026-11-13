//! The kind's picture in the frame (`dc_fx_annotations::look`): its fields as a layer.

use crate::TextBody;
use dc_fx_annotations::Placement;
use dc_fx_annotations::look::{Body, Moment, Read, read};
use dc_render::Layer;
use dc_render::{RenderError, TextMeasure};

/// How the annotations' picture reads this kind (the registration's table of kinds).
pub const READ: Read = read::<TextBody>;

impl Body for TextBody {
    fn check(&self, measure: &TextMeasure) -> Result<(), RenderError> {
        measure.check(&self.font)
    }

    fn layer(&self, placement: &Placement, _: &Moment<'_>) -> Option<Layer> {
        Some(Layer::Text(crate::evaluate(self, placement)))
    }
}
