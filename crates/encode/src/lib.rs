//! Live encoding while recording (H.264 video, FLAC sound), frames as PNG, MP4 and GIF export,
//! encoder self-test, export queue.

#![allow(
    unsafe_code,
    reason = "FFmpeg fields that ffmpeg-next does not wrap; each block has a SAFETY comment"
)]

pub mod audio;
pub mod codec;
pub mod encoder;
pub mod error;
pub mod frame;
pub mod mp4;
pub mod packet;
pub mod reader;
pub mod select;
pub mod still;

pub use audio::{AudioEncoder, AudioSettings};
pub use codec::Codec;
pub use encoder::{VideoEncoder, VideoSettings};
pub use error::EncodeError;
pub use frame::Nv12Frame;
pub use mp4::{Mp4Layout, Mp4Writer};
pub use packet::{Packet, StreamParameters};
pub use reader::Mp4Reader;
pub use select::{Selection, select};
pub use still::{Sources, frame_png, sheet_png};

/// Initializes FFmpeg once per process and keeps its own log to errors only:
/// encoders print statistics at `info`, and failures come back as [`EncodeError`] anyway.
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Error));
}
