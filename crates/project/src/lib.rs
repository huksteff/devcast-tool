//! Project folder: project/session/history files, lock, atomic writes, schema versions, app defaults.

pub mod atomic;
pub mod dir;
pub mod error;
pub mod migrate;

pub use dir::ProjectDir;
pub use error::{ProjectError, Result};
