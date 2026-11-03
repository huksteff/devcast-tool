//! The output frame as the preview shows it (`dc_ui_kit::stage`): its size in px (the canvas the
//! preview edits on; none without clips) and the name of its format for the toolbar.

use dc_core::{PixelSize, Project, StateError};
use dc_ui_kit::strings::editor as text;

use crate::{Aspect, Crop, canvas};

/// The output frame of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub canvas: Option<PixelSize>,
    /// «16:9», «Свободный» …
    pub format: &'static str,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            canvas: None,
            format: name(Aspect::default()),
        }
    }
}

/// The output frame of `project`; a broken crop state reads as the default and is the error.
pub fn frame(project: &Project) -> (Frame, Option<StateError>) {
    let (state, error) = match project.state::<Crop>() {
        Ok(state) => (state, None),
        Err(error) => (Default::default(), Some(error)),
    };
    let frame = Frame {
        canvas: canvas(project, &state.settings),
        format: name(state.settings.aspect),
    };
    (frame, error)
}

fn name(aspect: Aspect) -> &'static str {
    match aspect {
        Aspect::Free => text::ASPECT_FREE,
        Aspect::Source => text::ASPECT_SOURCE,
        Aspect::Wide16x9 => text::ASPECT_16_9,
        Aspect::Wide16x10 => text::ASPECT_16_10,
        Aspect::Standard4x3 => text::ASPECT_4_3,
    }
}
