//! Registry of all commands, batch apply, dry-run, one-line diff output, errors with hints; the
//! project's `evaluate` over all effects.

mod annotations;
pub mod apply;
pub mod argv;
pub mod autozoom;
mod devcast;
pub mod error;
pub mod evaluate;
pub mod plan;
pub mod recognition;
pub mod registry;
pub mod settings;
pub mod sound;
pub mod sound_file;

pub use apply::{Applied, apply, apply_commands, as_commands, duration_change};
pub use argv::parse_args;
pub use autozoom::{click_commands, click_zooms, no_click_zooms};
pub use error::OpsError;
pub use evaluate::{EvaluateError, Evaluator};
pub use plan::{Step, parse_plan};
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
