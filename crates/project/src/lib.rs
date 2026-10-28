//! Project folder: project/session/history files, lock, atomic writes, schema versions, app defaults.

pub mod atomic;
pub mod autosave;
pub mod dir;
pub mod error;
pub mod lock;
pub mod migrate;

pub use autosave::Autosave;
pub use dir::ProjectDir;
pub use error::{ProjectError, Result};
pub use lock::{LockOwner, ProjectLock};
