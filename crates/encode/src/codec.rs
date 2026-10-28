//! H.264 encoders in the order they are tried (decision 006): hardware first, then Media
//! Foundation, then x264 on the CPU, which works everywhere.

use std::fmt;

use ffmpeg_next as ffmpeg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    Nvenc,
    Qsv,
    Amf,
    MediaFoundation,
    X264,
}

impl Codec {
    /// Frames come from the CPU, so the GPU that captures and the one that encodes need not be
    /// the same: only one hardware encoder of the present vendor passes the self-test, and on
    /// a laptop with two GPUs the discrete NVIDIA one goes first.
    pub const ORDER: [Self; 5] = [
        Self::Nvenc,
        Self::Qsv,
        Self::Amf,
        Self::MediaFoundation,
        Self::X264,
    ];

    /// FFmpeg encoder name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nvenc => "h264_nvenc",
            Self::Qsv => "h264_qsv",
            Self::Amf => "h264_amf",
            Self::MediaFoundation => "h264_mf",
            Self::X264 => "libx264",
        }
    }

    /// "Good quality, fast" settings; forced key frames must be IDR so that every key frame
    /// starts a closed GOP and a segment can be decoded on its own.
    pub(crate) fn options(self) -> ffmpeg::Dictionary<'static> {
        let mut d = ffmpeg::Dictionary::new();
        match self {
            Self::Nvenc => {
                d.set("preset", "p4");
                d.set("rc", "vbr");
                d.set("cq", "21");
                d.set("forced-idr", "1");
            }
            Self::Qsv => {
                d.set("preset", "medium");
                d.set("forced_idr", "1");
            }
            Self::Amf => {
                d.set("quality", "balanced");
                d.set("forced_idr", "1");
            }
            Self::MediaFoundation => d.set("rate_control", "quality"),
            Self::X264 => {
                d.set("preset", "veryfast");
                d.set("crf", "20");
                d.set("forced-idr", "1");
            }
        }
        d
    }
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
