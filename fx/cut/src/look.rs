//! The clips' picture in the frame (`dc_render::look`): the video of the shown moment.

use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};

/// The entry of the list of pictures (`dc_ops::Evaluator`).
pub struct Picture;

#[derive(Debug)]
struct Clips;

impl Prepare for Picture {
    fn prepare(&self, _: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        Ok(Box::new(Clips))
    }
}

impl Look for Clips {
    fn draw(&self, frame: &mut Frame<'_>) {
        crate::evaluate(frame.project, frame.map, frame.t, frame.scene);
    }
}
