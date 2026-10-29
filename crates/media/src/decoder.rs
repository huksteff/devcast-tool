//! Software decoding (FFmpeg, frame threads) with frame-accurate seek (decision 003): seek back to
//! the keyframe at or before the target, then decode forward without copying until the frame
//! shown at the target. Only the returned frame is copied out, as tight NV12, into a buffer of
//! the [`FramePool`].
//!
//! A request after the last one within the same GOP decodes on from where the decoder stands
//! instead of seeking (playback, scrubbing forward).

use std::path::{Path, PathBuf};

use dc_core::{PixelSize, SourceTime};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::Rescale;

use crate::error::MediaError;
use crate::pool::{Buffer, FramePool};
use crate::probe::{MICROS, Probe};

/// A decoded frame: NV12 with the samples as the file stores them (matrix, range and chroma
/// siting in `Probe::color`), Y plane then interleaved UV, rows without padding.
#[derive(Debug)]
pub struct Frame {
    /// When the frame starts being shown.
    pub time: SourceTime,
    pub size: PixelSize,
    buffer: Buffer,
}

impl Frame {
    pub fn y(&self) -> &[u8] {
        &self.buffer.as_slice()[..self.y_len()]
    }

    /// `ceil(width/2)` UV pairs per row, `ceil(height/2)` rows.
    pub fn uv(&self) -> &[u8] {
        &self.buffer.as_slice()[self.y_len()..]
    }

    fn y_len(&self) -> usize {
        self.size.width as usize * self.size.height as usize
    }
}

/// Bytes of one NV12 frame of `size`.
pub fn nv12_len(size: PixelSize) -> usize {
    let (w, h) = (size.width as usize, size.height as usize);
    w * h + w.div_ceil(2) * 2 * h.div_ceil(2)
}

/// A decoded frame still inside FFmpeg, with its source time.
struct Decoded {
    time: SourceTime,
    frame: ffmpeg::frame::Video,
}

pub struct Decoder {
    path: PathBuf,
    probe: Probe,
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    time_base: ffmpeg::Rational,
    scaler: Option<ffmpeg::software::scaling::Context>,
    pool: FramePool,
    /// The last two frames the decoder gave, older first; both in presentation order.
    previous: Option<Decoded>,
    last: Option<Decoded>,
    ended: bool,
}

impl Decoder {
    /// Opens the video stream that `probe` describes (from [`crate::probe`] of the same file).
    pub fn open(path: &Path, probe: Probe, pool: FramePool) -> Result<Self, MediaError> {
        crate::init();
        let open_error = |source| MediaError::Open {
            path: path.to_path_buf(),
            source,
        };
        let input = ffmpeg::format::input(path).map_err(open_error)?;
        let stream = input
            .stream(probe.video)
            .ok_or_else(|| MediaError::NoVideo {
                path: path.to_path_buf(),
            })?;
        let time_base = stream.time_base();
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(open_error)?;
        context.set_threading(ffmpeg::threading::Config {
            kind: ffmpeg::threading::Type::Frame,
            count: 0,
        });
        let decoder = context.decoder().video().map_err(open_error)?;
        Ok(Self {
            path: path.to_path_buf(),
            probe,
            input,
            decoder,
            time_base,
            scaler: None,
            pool,
            previous: None,
            last: None,
            ended: false,
        })
    }

    pub fn probe(&self) -> &Probe {
        &self.probe
    }

    /// Exactly the frame shown at `t`: the last frame that starts at or before it. Before the
    /// first frame that is the first frame, after the end the last one.
    pub fn frame_at(&mut self, t: SourceTime) -> Result<Frame, MediaError> {
        if let (Some(previous), Some(last)) = (&self.previous, &self.last)
            && previous.time <= t
            && t < last.time
        {
            return self.copy_previous();
        }
        if !self.can_decode_on_to(t) {
            self.seek_to_keyframe(t)?;
        }
        loop {
            if !self.receive()? {
                return self.copy_last();
            }
            if let Some(last) = &self.last
                && last.time > t
            {
                return match self.previous {
                    Some(_) => self.copy_previous(),
                    None => self.copy_last(),
                };
            }
        }
    }

    /// The keyframe at or before `t`: fast, for scrubbing while the pointer moves.
    pub fn keyframe_at(&mut self, t: SourceTime) -> Result<Frame, MediaError> {
        let keyframe = self.probe.keyframe_before(t);
        self.seek_to_keyframe(keyframe)?;
        self.receive()?;
        self.copy_last()
    }

    /// True when decoding on from the last frame reaches `t` without passing a keyframe, which
    /// a seek would jump to directly.
    fn can_decode_on_to(&self, t: SourceTime) -> bool {
        match &self.last {
            Some(last) => last.time <= t && self.probe.keyframe_before(t) <= last.time,
            None => false,
        }
    }

    fn seek_to_keyframe(&mut self, t: SourceTime) -> Result<(), MediaError> {
        let keyframe = self.probe.keyframe_before(t);
        let target = self.probe.origin_micros + keyframe.micros();
        self.input
            .seek(target, ..target)
            .map_err(|source| self.decode_error(source))?;
        self.decoder.flush();
        self.previous = None;
        self.last = None;
        self.ended = false;
        Ok(())
    }

    /// Decodes the next frame into `last` (the old `last` becomes `previous`). False at the end
    /// of the stream.
    fn receive(&mut self) -> Result<bool, MediaError> {
        if self.ended {
            return Ok(false);
        }
        let mut frame = match self.previous.take() {
            Some(old) => old.frame,
            None => ffmpeg::frame::Video::empty(),
        };
        loop {
            match self.decoder.receive_frame(&mut frame) {
                Ok(()) => break,
                Err(ffmpeg::Error::Other {
                    errno: ffmpeg::error::EAGAIN,
                }) => self.feed()?,
                Err(ffmpeg::Error::Eof) => {
                    self.ended = true;
                    return Ok(false);
                }
                Err(source) => return Err(self.decode_error(source)),
            }
        }
        frame.set_format(without_range_mark(frame.format()));
        let ts = frame
            .timestamp()
            .ok_or_else(|| self.decode_error(ffmpeg::Error::InvalidData))?;
        let time =
            SourceTime::from_micros((ts - self.probe.first_pts).rescale(self.time_base, MICROS));
        self.previous = self.last.take();
        self.last = Some(Decoded { time, frame });
        Ok(true)
    }

    /// Sends the next packet of the video stream, or the end of the stream.
    fn feed(&mut self) -> Result<(), MediaError> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() != self.probe.video => {}
                Ok(()) => {
                    return self
                        .decoder
                        .send_packet(&packet)
                        .map_err(|source| self.decode_error(source));
                }
                Err(ffmpeg::Error::Eof) => {
                    return self
                        .decoder
                        .send_eof()
                        .map_err(|source| self.decode_error(source));
                }
                Err(source) => return Err(self.decode_error(source)),
            }
        }
    }

    fn copy_previous(&mut self) -> Result<Frame, MediaError> {
        let decoded = self.previous.take();
        let frame = decoded.as_ref().map(|d| self.copy(d)).transpose();
        self.previous = decoded;
        frame?.ok_or_else(|| self.no_frames())
    }

    fn copy_last(&mut self) -> Result<Frame, MediaError> {
        let decoded = self.last.take();
        let frame = decoded.as_ref().map(|d| self.copy(d)).transpose();
        self.last = decoded;
        frame?.ok_or_else(|| self.no_frames())
    }

    /// Copies `decoded` out as tight NV12, converting other pixel formats first.
    fn copy(&mut self, decoded: &Decoded) -> Result<Frame, MediaError> {
        let source = &decoded.frame;
        let mut converted = ffmpeg::frame::Video::empty();
        let nv12 = match source.format() {
            ffmpeg::format::Pixel::NV12 => source,
            format => {
                let mut scaler = match self.scaler.take() {
                    Some(scaler) => scaler,
                    None => ffmpeg::software::scaling::Context::get(
                        format,
                        source.width(),
                        source.height(),
                        ffmpeg::format::Pixel::NV12,
                        source.width(),
                        source.height(),
                        ffmpeg::software::scaling::Flags::POINT,
                    )
                    .map_err(|e| self.decode_error(e))?,
                };
                let converting = scaler.run(source, &mut converted);
                self.scaler = Some(scaler);
                converting.map_err(|e| self.decode_error(e))?;
                &converted
            }
        };
        let size = PixelSize {
            width: nv12.width(),
            height: nv12.height(),
        };
        let mut buffer = self.pool.take(nv12_len(size))?;
        let (w, h) = (size.width as usize, size.height as usize);
        let (y, uv) = buffer.as_mut_slice().split_at_mut(w * h);
        pack_plane(nv12, 0, w, y);
        pack_plane(nv12, 1, w.div_ceil(2) * 2, uv);
        Ok(Frame {
            time: decoded.time,
            size,
            buffer,
        })
    }

    fn decode_error(&self, source: ffmpeg::Error) -> MediaError {
        MediaError::Decode {
            path: self.path.clone(),
            source,
        }
    }

    fn no_frames(&self) -> MediaError {
        MediaError::NoFrames {
            path: self.path.clone(),
        }
    }
}

/// Copies plane `plane` of `frame` row by row into `out`, `row_bytes` per row.
fn pack_plane(frame: &ffmpeg::frame::Video, plane: usize, row_bytes: usize, out: &mut [u8]) {
    let stride = frame.stride(plane);
    let data = frame.data(plane);
    for (row, target) in out.chunks_exact_mut(row_bytes).enumerate() {
        target.copy_from_slice(&data[row * stride..row * stride + row_bytes]);
    }
}

/// The J formats are YUV formats marked full range. The range is in `Probe::color`, so the copy
/// to NV12 keeps the samples as decoded instead of letting swscale squeeze them to limited range.
fn without_range_mark(format: ffmpeg::format::Pixel) -> ffmpeg::format::Pixel {
    use ffmpeg::format::Pixel;
    match format {
        Pixel::YUVJ420P => Pixel::YUV420P,
        Pixel::YUVJ422P => Pixel::YUV422P,
        Pixel::YUVJ444P => Pixel::YUV444P,
        Pixel::YUVJ440P => Pixel::YUV440P,
        Pixel::YUVJ411P => Pixel::YUV411P,
        other => other,
    }
}

#[cfg(test)]
#[path = "decoder_tests.rs"]
mod tests;
