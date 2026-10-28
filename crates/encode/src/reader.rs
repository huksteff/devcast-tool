//! Reading encoded packets back from an MP4 file (plain or fragmented), without decoding.

use std::path::{Path, PathBuf};

use ffmpeg_next as ffmpeg;

use crate::error::EncodeError;
use crate::packet::{Packet, StreamParameters};

pub struct Mp4Reader {
    input: ffmpeg::format::context::Input,
    path: PathBuf,
    stream: usize,
    stream_time_base: ffmpeg::Rational,
    parameters: StreamParameters,
}

impl Mp4Reader {
    /// Opens the stream of a one-stream file `path` (a recording track); packets come in
    /// `time_base` (seconds per tick).
    pub fn open(path: &Path, (num, den): (i32, i32)) -> Result<Self, EncodeError> {
        crate::init();
        let input = ffmpeg::format::input(path).map_err(EncodeError::file("открыть", path))?;
        let stream = input.stream(0).ok_or_else(|| {
            EncodeError::file("найти дорожку в", path)(ffmpeg::Error::StreamNotFound)
        })?;
        let parameters = StreamParameters {
            codec: stream.parameters(),
            time_base: ffmpeg::Rational::new(num, den),
        };
        Ok(Self {
            stream: stream.index(),
            stream_time_base: stream.time_base(),
            parameters,
            path: path.to_path_buf(),
            input,
        })
    }

    pub fn parameters(&self) -> &StreamParameters {
        &self.parameters
    }

    /// The next packet, `None` at the end of the file.
    pub fn read(&mut self) -> Result<Option<Packet>, EncodeError> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => return Ok(None),
                Err(e) => return Err(EncodeError::file("прочитать", &self.path)(e)),
            }
            if packet.stream() != self.stream {
                continue;
            }
            packet.rescale_ts(self.stream_time_base, self.parameters.time_base);
            let pts = packet.pts().ok_or_else(|| {
                EncodeError::file("прочитать время пакета из", &self.path)(
                    ffmpeg::Error::InvalidData,
                )
            })?;
            return Ok(Some(Packet::new(packet, pts)));
        }
    }
}
