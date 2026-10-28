//! Whisper models, transcription with word timestamps, segmentation into caption cues.

pub mod download;
pub mod error;
pub mod hash;
pub mod model;
#[cfg(test)]
mod test_server;

pub use download::{Progress, download, ensure};
pub use error::{Result, TranscribeError};
pub use model::{Model, models_dir};
