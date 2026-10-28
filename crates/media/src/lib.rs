//! Probe, keyframe index, hardware decoding, frame-accurate seek, preview frames.

pub mod cache;
pub mod error;
pub mod probe;

#[cfg(test)]
mod test_util;

pub use cache::{cached_probe, probe_cache_path};
pub use error::MediaError;
pub use probe::{Probe, StreamInfo, StreamKind, probe};

/// Initializes FFmpeg once per process and keeps its log to errors only: failures come back as
/// [`MediaError`] anyway.
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Error));
}
