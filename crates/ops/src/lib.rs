//! Registry of all commands, batch apply, dry-run, one-line diff output, errors with hints; the
//! project's `evaluate` over all effects.

mod annotations;
pub mod apply;
pub mod argv;
pub mod autozoom;
pub mod clip_file;
mod devcast;
pub mod error;
pub mod evaluate;
pub mod export;
pub mod export_jobs;
pub mod fonts;
pub mod live;
mod live_app;
pub mod new_project;
pub mod picture_file;
pub mod plan;
pub mod recognition;
pub mod registry;
pub mod scene;
pub mod settings;
pub mod sound;
pub mod sound_file;

pub use apply::{Applied, apply, apply_commands, as_commands, duration_change};
pub use argv::parse_args;
pub use autozoom::{click_commands, click_zooms, no_click_zooms};
pub use clip_file::{ClipImport, ClipImportError, import_clips};
pub use error::OpsError;
pub use evaluate::{EvaluateError, Evaluator};
pub use fonts::{FontError, FontImport, import_font};
pub use new_project::{
    EMPTY_NAME, NewProjectError, add_clips, empty_root, new_empty, new_project, new_projects,
    project_roots,
};
pub use picture_file::{PictureImport, import_picture};
pub use plan::{Commands, Step, build_with, parse_plan, step_name};
pub use registry::{RESERVED_FIELDS, Registry};
pub use settings::{settings_commands, settings_steps};
pub use sound_file::{DiscardError, SoundImport, SoundPlace, import_sound};

#[cfg(test)]
mod annotations_blur_tests;
#[cfg(test)]
mod annotations_drawing_tests;
#[cfg(test)]
mod annotations_image_tests;
#[cfg(test)]
mod annotations_shape_tests;
#[cfg(test)]
mod annotations_tests;
#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod tests;
