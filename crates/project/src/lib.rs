//! Project folder: project/session/history files, lock, atomic writes, schema versions, app defaults.

pub mod atomic;
pub mod autosave;
pub mod cache;
pub mod copy_in;
pub mod defaults;
pub mod dir;
pub mod error;
pub mod history_log;
pub mod lock;
pub mod migrate;
pub mod settings;
pub mod working;

pub use autosave::Autosave;
pub use cache::FileStamp;
pub use copy_in::{CopiedFile, copy_in};
pub use defaults::Defaults;
pub use dir::ProjectDir;
pub use error::{ProjectError, Result};
pub use history_log::HistoryLog;
pub use lock::{LockOwner, ProjectLock};
pub use working::Working;
