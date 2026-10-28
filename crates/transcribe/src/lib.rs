//! Whisper models, transcription with word timestamps, segmentation into caption cues.

pub mod cache;
pub mod cues;
pub mod download;
pub mod error;
pub mod hash;
pub mod model;
pub mod recognize;
pub mod speech;
#[cfg(test)]
mod test_server;
pub mod words;

pub use cache::{read_words, words_cache_path, write_words};
pub use cues::{Cue, cues};
pub use download::{Progress, download, ensure};
pub use error::{Result, TranscribeError};
pub use model::{Model, models_dir};
pub use recognize::{Recognized, WHISPER_RATE, recognize};
pub use speech::speech_samples;
pub use words::Word;
