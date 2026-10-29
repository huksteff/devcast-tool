//! Probe, keyframe index, software decoding, frame-accurate seek, preview frames, pictures.

pub mod cache;
pub mod decoder;
pub mod error;
pub mod import;
pub mod picture;
pub mod pool;
pub mod preview;
pub mod probe;
pub mod seeker;

#[cfg(test)]
mod test_util;

pub use cache::{cached_probe, probe_cache_path};
pub use decoder::{Decoder, Frame, nv12_len};
pub use error::MediaError;
pub use import::{VIDEO_EXTENSIONS, import, import_dry_run};
pub use picture::{PICTURE_MAX_BYTES, Picture, PictureFrame, decode_picture};
pub use pool::{Buffer, FramePool};
pub use preview::{PREVIEW_MAX, preview, project_preview};
pub use probe::{Probe, StreamInfo, StreamKind, probe};
pub use seeker::{Precision, Seeker};

/// Initializes FFmpeg once per process and keeps its log to errors only: failures come back as
/// [`MediaError`] anyway.
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Error));
}
