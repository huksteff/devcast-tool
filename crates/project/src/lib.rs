//! Project folder: project/session/history files, lock, atomic writes, schema versions, app defaults.

pub mod atomic;
pub mod autosave;
pub mod defaults;
pub mod dir;
pub mod error;
pub mod history_log;
pub mod lock;
pub mod migrate;

pub use autosave::Autosave;
pub use defaults::Defaults;
pub use dir::ProjectDir;
pub use error::{ProjectError, Result};
pub use history_log::HistoryLog;
pub use lock::{LockOwner, ProjectLock};
