//! Registry of all commands, batch apply, dry-run, one-line diff output, errors with hints; the
//! project's `evaluate` over all effects.

mod annotations;
pub mod apply;
pub mod argv;
pub mod error;
pub mod evaluate;
pub mod plan;
pub mod registry;
pub mod settings;

pub use apply::{Applied, apply, apply_commands, as_commands, duration_change};
pub use argv::parse_args;
pub use error::OpsError;
pub use evaluate::{EvaluateError, Evaluator};
pub use plan::{Step, parse_plan};
pub use registry::{RESERVED_FIELDS, Registry};
pub use settings::{settings_commands, settings_steps};

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
