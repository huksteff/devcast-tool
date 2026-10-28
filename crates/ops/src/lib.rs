//! Registry of all commands, batch apply, dry-run, one-line diff output, errors with hints.

pub mod apply;
pub mod argv;
pub mod error;
pub mod plan;
pub mod registry;

pub use apply::{Applied, apply};
pub use argv::parse_args;
pub use error::OpsError;
pub use plan::{Step, parse_plan};
pub use registry::Registry;

#[cfg(test)]
mod tests;
