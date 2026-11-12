//! The camera of the zooms in the frame (`dc_render::look`): simulated once over the whole output.

use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};

use crate::{CameraTrack, ZoomEffect};

/// The entry of the list of pictures (`dc_ops::Evaluator`).
pub struct Picture;

impl Prepare for Picture {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        let state = inputs.project.state::<ZoomEffect>()?;
        let camera = CameraTrack::new(
            inputs.project,
            &state,
            inputs.map,
            inputs.canvas,
            inputs.cursors,
        );
        Ok(Box::new(camera))
    }
}

impl Look for CameraTrack {
    fn draw(&self, frame: &mut Frame<'_>) {
        self.evaluate(frame.t, frame.scene);
    }
}
