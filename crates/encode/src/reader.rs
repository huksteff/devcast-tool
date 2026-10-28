//! Reading encoded video packets back from an MP4 file (plain or fragmented), without decoding.

use std::path::{Path, PathBuf};

use ffmpeg_next as ffmpeg;

use crate::error::EncodeError;
use crate::packet::{Packet, VideoParameters};

pub struct Mp4Reader {
    input: ffmpeg::format::context::Input,
    path: PathBuf,
    stream: usize,
    stream_time_base: ffmpeg::Rational,
    video: VideoParameters,
}

impl Mp4Reader {
    /// Opens the first video stream of `path`; packets come in `time_base` (seconds per tick).
    pub fn open(path: &Path, (num, den): (i32, i32)) -> Result<Self, EncodeError> {
        crate::init();
        let input = ffmpeg::format::input(path).map_err(EncodeError::file("открыть", path))?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or_else(|| {
                EncodeError::file("найти видео в", path)(ffmpeg::Error::StreamNotFound)
            })?;
        let video = VideoParameters {
            codec: stream.parameters(),
            time_base: ffmpeg::Rational::new(num, den),
        };
        Ok(Self {
            stream: stream.index(),
            stream_time_base: stream.time_base(),
            video,
            path: path.to_path_buf(),
            input,
        })
    }

    pub fn parameters(&self) -> &VideoParameters {
        &self.video
    }

    /// The next packet of the video stream, `None` at the end of the file.
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
            packet.rescale_ts(self.stream_time_base, self.video.time_base);
            let pts = packet.pts().ok_or_else(|| {
                EncodeError::file("прочитать время кадра из", &self.path)(
                    ffmpeg::Error::InvalidData,
                )
            })?;
            return Ok(Some(Packet::new(packet, pts)));
        }
    }
}
