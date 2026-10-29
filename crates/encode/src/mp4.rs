//! An MP4 file: one stream (a recording's H.264 video or FLAC sound) or several (an export's
//! video and sound).

use std::path::{Path, PathBuf};

use ffmpeg_next as ffmpeg;

use crate::error::EncodeError;
use crate::packet::{Packet, StreamParameters};

/// How the file is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4Layout {
    /// One index at the end: the usual file for players and the editor.
    Plain,
    /// Fragmented MP4: a fragment per video key frame (sound: one at the end), no index at the
    /// end. Used for recording segments.
    Fragmented,
}

pub struct Mp4Writer {
    out: ffmpeg::format::context::Output,
    path: PathBuf,
    /// Per stream: the time base packets come in and the one the file keeps.
    time_bases: Vec<(ffmpeg::Rational, ffmpeg::Rational)>,
}

impl Mp4Writer {
    /// Creates the file with one stream and writes its header. Packets come in `stream`'s time
    /// base.
    pub fn create(
        path: &Path,
        stream: &StreamParameters,
        layout: Mp4Layout,
    ) -> Result<Self, EncodeError> {
        Self::with_streams(path, &[stream], layout)
    }

    /// Creates the file with `streams` in this order and writes its header.
    pub fn with_streams(
        path: &Path,
        streams: &[&StreamParameters],
        layout: Mp4Layout,
    ) -> Result<Self, EncodeError> {
        crate::init();
        let mut out =
            ffmpeg::format::output_as(path, "mp4").map_err(EncodeError::file("создать", path))?;
        for stream in streams {
            let mut added = out
                .add_stream(None::<ffmpeg::Codec>)
                .map_err(EncodeError::file("создать дорожку в", path))?;
            added.set_parameters(stream.codec.clone());
            added.set_time_base(stream.time_base);
        }
        let mut options = ffmpeg::Dictionary::new();
        if layout == Mp4Layout::Fragmented {
            options.set("movflags", "frag_keyframe+empty_moov+default_base_moof");
        }
        out.write_header_with(options)
            .map_err(EncodeError::file("записать заголовок в", path))?;
        let time_bases = streams
            .iter()
            .zip(out.streams())
            .map(|(stream, kept)| (stream.time_base, kept.time_base()))
            .collect();
        Ok(Self {
            out,
            path: path.to_path_buf(),
            time_bases,
        })
    }

    /// Writes a packet of the first stream.
    pub fn write(&mut self, packet: Packet) -> Result<(), EncodeError> {
        self.write_to(0, packet)
    }

    /// Writes a packet of stream `index`; FFmpeg interleaves the streams by time.
    pub fn write_to(&mut self, index: usize, packet: Packet) -> Result<(), EncodeError> {
        let (from, to) = *self.time_bases.get(index).ok_or_else(|| {
            EncodeError::file("записать пакет в", &self.path)(
                ffmpeg::Error::StreamNotFound,
            )
        })?;
        let mut inner = packet.inner;
        inner.set_stream(index);
        inner.rescale_ts(from, to);
        inner
            .write_interleaved(&mut self.out)
            .map_err(EncodeError::file("записать пакет в", &self.path))?;
        Ok(())
    }

    /// Writes the index; without it the file does not open.
    pub fn finish(mut self) -> Result<(), EncodeError> {
        self.out
            .write_trailer()
            .map_err(EncodeError::file("дописать", &self.path))
    }
}
