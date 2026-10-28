//! Live encoding while recording, MP4 and GIF export, encoder self-test, export queue.

#![allow(
    unsafe_code,
    reason = "FFmpeg fields that ffmpeg-next does not wrap; each block has a SAFETY comment"
)]

pub mod codec;
pub mod encoder;
pub mod error;
pub mod frame;
pub mod mp4;
pub mod packet;
pub mod select;

pub use codec::Codec;
pub use encoder::{VideoEncoder, VideoSettings};
pub use error::EncodeError;
pub use frame::Nv12Frame;
pub use mp4::Mp4Writer;
pub use packet::{Packet, VideoParameters};
pub use select::{Selection, select};

/// Initializes FFmpeg once per process and keeps its own log to errors only:
/// encoders print statistics at `info`, and failures come back as [`EncodeError`] anyway.
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Error));
}
