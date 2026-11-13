//! The cursor in the frame (`dc_render::look`): its paths are built once; each frame it leaves
//! where it points (`Frame::pointer`) for the callout.

use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};

use crate::{Cursor, Prepared};

/// The entry of the list of pictures (`dc_ops::Evaluator`).
pub struct Picture;

impl Prepare for Picture {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        let state = inputs.project.state::<Cursor>()?;
        let prepared = Prepared::new(inputs.project, state, inputs.cursors);
        Ok(Box::new(prepared))
    }
}

impl Look for Prepared {
    fn draw(&self, frame: &mut Frame<'_>) {
        crate::evaluate(self, frame.project, frame.map, frame.t, frame.scene);
        let size = frame.scene.size;
        frame.pointer = crate::pointer(self, frame.project, frame.map, frame.t, size);
    }
}
