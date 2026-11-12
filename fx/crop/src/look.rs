//! The output frame every picture is built on (`dc_render::look`): the canvas after the crop.

use dc_core::{PixelSize, Project, StateError};

use crate::Crop;

/// The canvas of `project`, px; none without clips.
pub fn canvas(project: &Project) -> Result<Option<PixelSize>, StateError> {
    let crop = project.state::<Crop>()?;
    Ok(crate::canvas(project, &crop.settings))
}
