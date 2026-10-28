//! An MP4 file with one H.264 video stream.

use std::path::{Path, PathBuf};

use ffmpeg_next as ffmpeg;

use crate::error::EncodeError;
use crate::packet::{Packet, VideoParameters};

/// How the file is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4Layout {
    /// One index at the end: the usual file for players and the editor.
    Plain,
    /// Fragmented MP4: a fragment per key frame, no index at the end. Used for recording
    /// segments.
    Fragmented,
}

pub struct Mp4Writer {
    out: ffmpeg::format::context::Output,
    path: PathBuf,
    packet_time_base: ffmpeg::Rational,
    stream_time_base: ffmpeg::Rational,
}

impl Mp4Writer {
    /// Creates the file and writes its header. Packets come in `video`'s time base.
    pub fn create(
        path: &Path,
        video: &VideoParameters,
        layout: Mp4Layout,
    ) -> Result<Self, EncodeError> {
        crate::init();
        let mut out =
            ffmpeg::format::output_as(path, "mp4").map_err(EncodeError::file("создать", path))?;
        {
            let mut stream = out
                .add_stream(None::<ffmpeg::Codec>)
                .map_err(EncodeError::file("создать дорожку в", path))?;
            stream.set_parameters(video.codec.clone());
            stream.set_time_base(video.time_base);
        }
        let mut options = ffmpeg::Dictionary::new();
        if layout == Mp4Layout::Fragmented {
            options.set("movflags", "frag_keyframe+empty_moov+default_base_moof");
        }
        out.write_header_with(options)
            .map_err(EncodeError::file("записать заголовок в", path))?;
        let stream_time_base = out.stream(0).map(|s| s.time_base()).ok_or_else(|| {
            EncodeError::file("создать дорожку в", path)(
                ffmpeg::Error::StreamNotFound,
            )
        })?;
        Ok(Self {
            out,
            path: path.to_path_buf(),
            packet_time_base: video.time_base,
            stream_time_base,
        })
    }

    pub fn write(&mut self, packet: Packet) -> Result<(), EncodeError> {
        let mut inner = packet.inner;
        inner.set_stream(0);
        inner.rescale_ts(self.packet_time_base, self.stream_time_base);
        inner
            .write(&mut self.out)
            .map_err(EncodeError::file("записать кадр в", &self.path))?;
        Ok(())
    }

    /// Writes the index; without it the file does not open.
    pub fn finish(mut self) -> Result<(), EncodeError> {
        self.out
            .write_trailer()
            .map_err(EncodeError::file("дописать", &self.path))
    }
}
