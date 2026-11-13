//! The pressed-keys plate in the frame (`dc_render::look`): the shortcuts are found once.

use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};

use crate::{Keys, Prepared};

/// The entry of the list of pictures (`dc_ops::Evaluator`).
pub struct Picture;

impl Prepare for Picture {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        let state = inputs.project.state::<Keys>()?;
        Ok(Box::new(Prepared::new(state, inputs.keys, inputs.map)))
    }
}

impl Look for Prepared {
    fn draw(&self, frame: &mut Frame<'_>) {
        crate::evaluate(self, frame.t, frame.scene);
    }
}
