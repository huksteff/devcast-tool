//! Registry of all commands, batch apply, dry-run, one-line diff output, errors with hints.

pub mod apply;
pub mod argv;
pub mod error;
pub mod plan;
pub mod registry;

pub use apply::{Applied, apply, duration_change};
pub use argv::parse_args;
pub use error::OpsError;
pub use plan::{Step, parse_plan};
pub use registry::{RESERVED_FIELDS, Registry};

#[cfg(test)]
mod tests;
