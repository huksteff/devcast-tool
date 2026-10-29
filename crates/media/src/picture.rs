//! Pictures for sprites (image annotations, custom cursors): PNG, JPEG, WebP and GIF, animated
//! GIF and WebP included, decoded by FFmpeg into straight-alpha RGBA. Each frame comes whole, as
//! shown (the decoder applies GIF disposal), with how long it is shown.

use std::path::Path;

use dc_core::{PixelSize, TimeDelta};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::Rescale;

use crate::error::MediaError;
use crate::probe::MICROS;

/// Decoded frames may take this much memory at most: a long animation of a big picture is
/// refused instead of filling the memory.
pub const PICTURE_MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub size: PixelSize,
    /// One for a still picture.
    pub frames: Vec<PictureFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureFrame {
    /// RGBA, straight alpha, rows without padding.
    pub rgba: Vec<u8>,
    /// How long the frame is shown; zero for a still picture.
    pub duration: TimeDelta,
}

pub fn decode_picture(path: &Path) -> Result<Picture, MediaError> {
    crate::init();
    let error = |source| MediaError::Picture {
        path: path.to_path_buf(),
        source,
    };
    let mut input = ffmpeg::format::input(path).map_err(error)?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)
        .map_err(error)?;
    let (index, time_base) = (stream.index(), stream.time_base());
    let context =
        ffmpeg::codec::context::Context::from_parameters(stream.parameters()).map_err(error)?;
    let mut decoder = context.decoder().video().map_err(error)?;
    let mut frames = Frames::new(path, time_base);
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) if packet.stream() == index => {
                decoder.send_packet(&packet).map_err(error)?;
                frames.receive(&mut decoder)?;
            }
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => break,
            Err(source) => return Err(error(source)),
        }
    }
    decoder.send_eof().map_err(error)?;
    frames.receive(&mut decoder)?;
    frames.finish()
}

/// Frames converted to RGBA as they come out of the decoder.
struct Frames<'p> {
    path: &'p Path,
    time_base: ffmpeg::Rational,
    scaler: Option<ffmpeg::software::scaling::Context>,
    size: Option<PixelSize>,
    frames: Vec<PictureFrame>,
}

impl<'p> Frames<'p> {
    fn new(path: &'p Path, time_base: ffmpeg::Rational) -> Self {
        Self {
            path,
            time_base,
            scaler: None,
            size: None,
            frames: Vec::new(),
        }
    }

    fn receive(&mut self, decoder: &mut ffmpeg::decoder::Video) -> Result<(), MediaError> {
        let mut decoded = ffmpeg::frame::Video::empty();
        loop {
            match decoder.receive_frame(&mut decoded) {
                Ok(()) => {}
                Err(
                    ffmpeg::Error::Eof
                    | ffmpeg::Error::Other {
                        errno: ffmpeg::error::EAGAIN,
                    },
                ) => {
                    return Ok(());
                }
                Err(source) => return Err(self.error(source)),
            }
            let size = PixelSize {
                width: decoded.width(),
                height: decoded.height(),
            };
            if *self.size.get_or_insert(size) != size {
                return Err(self.error(ffmpeg::Error::InvalidData));
            }
            let bytes = (self.frames.len() + 1) * size.width as usize * size.height as usize * 4;
            if bytes > PICTURE_MAX_BYTES {
                return Err(MediaError::PictureTooBig {
                    path: self.path.to_path_buf(),
                    limit_mb: PICTURE_MAX_BYTES / 1024 / 1024,
                });
            }
            let rgba = self.convert(&decoded)?;
            let duration = decoded.packet().duration.rescale(self.time_base, MICROS);
            self.frames.push(PictureFrame {
                rgba,
                duration: TimeDelta::from_micros(duration),
            });
        }
    }

    fn convert(&mut self, decoded: &ffmpeg::frame::Video) -> Result<Vec<u8>, MediaError> {
        let (w, h) = (decoded.width(), decoded.height());
        let mut scaler = match self.scaler.take() {
            Some(scaler) => scaler,
            None => ffmpeg::software::scaling::Context::get(
                decoded.format(),
                w,
                h,
                ffmpeg::format::Pixel::RGBA,
                w,
                h,
                ffmpeg::software::scaling::Flags::POINT,
            )
            .map_err(|e| self.error(e))?,
        };
        let mut rgba = ffmpeg::frame::Video::empty();
        let scaled = scaler.run(decoded, &mut rgba);
        self.scaler = Some(scaler);
        scaled.map_err(|e| self.error(e))?;
        let row = w as usize * 4;
        let stride = rgba.stride(0);
        let data = rgba.data(0);
        Ok((0..h as usize)
            .flat_map(|y| &data[y * stride..y * stride + row])
            .copied()
            .collect())
    }

    fn finish(self) -> Result<Picture, MediaError> {
        match self.size {
            Some(size) => {
                let mut frames = self.frames;
                if let [still] = frames.as_mut_slice() {
                    still.duration = TimeDelta::ZERO;
                }
                Ok(Picture { size, frames })
            }
            None => Err(self.error(ffmpeg::Error::InvalidData)),
        }
    }

    fn error(&self, source: ffmpeg::Error) -> MediaError {
        MediaError::Picture {
            path: self.path.to_path_buf(),
            source,
        }
    }
}

#[cfg(test)]
#[path = "picture_tests.rs"]
mod tests;
