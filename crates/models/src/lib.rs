//! Whisper model files: which there are, where they live and their resumable, hash-checked
//! download. No Whisper here: the app window links this crate to download models without
//! linking whisper.cpp and Vulkan (docs/decisions/018-recognition-process.md).

pub mod download;
pub mod error;
pub mod hash;
pub mod model;
#[cfg(test)]
mod test_server;

pub use download::{Progress, download, ensure};
pub use error::{ModelsError, Result};
pub use model::{Model, models_dir};
