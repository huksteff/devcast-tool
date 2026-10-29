//! Audio decoding and resampling, mixing by the time map, levels, EBU R128, denoise, waveform,
//! silence.
//!
//! Every track is brought to one format: [`RATE`] Hz, [`CHANNELS`] interleaved `f32` channels.
//! A position in it is a frame index ([`frame_at`]).

mod denoise;
pub mod error;
pub mod levels;
pub mod loudness;
pub mod mix;
pub mod peaks;
pub mod reader;
pub mod silence;
pub mod track;

mod limiter;
mod part;
mod rate;
mod stream;
mod tempo;
#[cfg(test)]
mod test_util;

pub use error::AudioError;
pub use levels::{ChannelLevel, ClipLevels, GAIN_MAX, Levels};
pub use limiter::CEILING;
pub use loudness::{LoudnessCache, Measurement, Meter, TARGET};
pub use mix::Mixer;
pub use part::CROSSFADE;
pub use reader::TrackReader;
pub use track::{SourceTrack, tracks};

/// Sample rate of all decoded and mixed sound.
pub const RATE: u32 = 48_000;

/// Channels of all decoded and mixed sound (stereo, interleaved).
pub const CHANNELS: usize = 2;

/// Frame of [`RATE`] at `micros` microseconds, rounded to the nearest.
pub fn frame_at(micros: i64) -> i64 {
    let scaled = i128::from(micros) * i128::from(RATE);
    ((scaled + 500_000 * scaled.signum()) / 1_000_000) as i64
}

/// Initializes FFmpeg once per process; its own log is limited to errors, which come back as
/// [`AudioError`] anyway.
fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Error));
}
