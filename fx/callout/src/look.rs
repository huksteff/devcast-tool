//! The callout in the frame (`dc_render::look`): it hangs on the pointer the cursor left in the
//! frame, so it comes after the cursor in the list of pictures.

use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};

use crate::{Callout, CalloutState};

/// The entry of the list of pictures (`dc_ops::Evaluator`).
pub struct Picture;

impl Prepare for Picture {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        Ok(Box::new(inputs.project.state::<Callout>()?))
    }
}

impl Look for CalloutState {
    fn draw(&self, frame: &mut Frame<'_>) {
        crate::evaluate(self, frame.map, frame.t, frame.pointer, frame.scene);
    }
}
