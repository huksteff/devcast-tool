//! Captions in the frame (`dc_render::look`): the cues are broken into lines once (decision 016).

use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};

use crate::{CaptionsEffect, Prepared};

/// The entry of the list of pictures (`dc_ops::Evaluator`).
pub struct Picture;

impl Prepare for Picture {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        let state = inputs.project.state::<CaptionsEffect>()?;
        let prepared = Prepared::new(state, inputs.canvas, inputs.measure)?;
        Ok(Box::new(prepared))
    }
}

impl Look for Prepared {
    fn draw(&self, frame: &mut Frame<'_>) {
        crate::evaluate(self, frame.map, frame.t, frame.scene);
    }
}
