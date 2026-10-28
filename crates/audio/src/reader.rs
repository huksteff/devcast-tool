//! Random access to one sound track in the common format: frame `n` of the reader is at source
//! time `n / RATE` seconds, that is stream time (FFmpeg timestamps) `n / RATE` after the track's
//! origin. Outside the stream it is silence. Reading forward decodes on; a jump back or far ahead
//! seeks. Inside, frames count from the start of the stream's clock.

use crate::error::AudioError;
use crate::rate::RateConverter;
use crate::stream::StreamDecoder;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE};

/// A jump ahead further than this seeks instead of decoding through, frames.
const SEEK_AHEAD: i64 = RATE as i64;

/// A seek lands this much before the asked frame, so that the resampler is settled there.
const SEEK_LEAD: f64 = 0.1;

pub struct TrackReader {
    stream: StreamDecoder,
    /// Stream frame of source frame 0.
    origin: i64,
    converter: Option<RateConverter>,
    native: Vec<f32>,
    /// Decoded frames in the common format, from `position`.
    buffer: Vec<f32>,
    position: i64,
    /// The next decoded frame starts a new run (after opening or a seek): it sets `position`.
    fresh: bool,
    ended: bool,
}

impl TrackReader {
    pub fn open(track: &SourceTrack) -> Result<Self, AudioError> {
        let stream = StreamDecoder::open(track)?;
        let converter = match stream.rate() {
            RATE => None,
            rate => Some(RateConverter::new(rate, &track.path)?),
        };
        Ok(Self {
            stream,
            origin: crate::frame_at(track.origin),
            converter,
            native: Vec::new(),
            buffer: Vec::new(),
            position: 0,
            fresh: true,
            ended: false,
        })
    }

    /// Fills `out` (interleaved, [`CHANNELS`]) with the frames from source frame `start` on.
    pub fn fill(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        self.fill_stream(start + self.origin, out)
    }

    /// Like [`Self::fill`], but says how many frames of `out` are before the end of the
    /// stream; `None` when `start` is past it.
    pub fn fill_until_end(
        &mut self,
        start: i64,
        out: &mut [f32],
    ) -> Result<Option<usize>, AudioError> {
        let start = start + self.origin;
        self.fill_stream(start, out)?;
        let frames = (out.len() / CHANNELS) as i64;
        if !self.ended {
            return Ok(Some(frames as usize));
        }
        let left = (self.buffered_end() - start).min(frames);
        Ok((left > 0).then_some(left as usize))
    }

    /// [`Self::fill`] from stream frame `start`.
    fn fill_stream(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        let end = start + (out.len() / CHANNELS) as i64;
        if start < self.position || start > self.buffered_end() + SEEK_AHEAD {
            self.seek(start)?;
        }
        while !self.ended && (self.fresh || self.buffered_end() < end) {
            self.decode_more()?;
            self.discard_before(start);
        }
        self.discard_before(start);
        out.fill(0.0);
        let first = self.position.max(start);
        let last = self.buffered_end().min(end);
        if first < last {
            let (from, to) = ((first - start) as usize, (last - start) as usize);
            let skip = (first - self.position) as usize;
            out[from * CHANNELS..to * CHANNELS]
                .copy_from_slice(&self.buffer[skip * CHANNELS..(skip + to - from) * CHANNELS]);
        }
        Ok(())
    }

    fn buffered_end(&self) -> i64 {
        self.position + (self.buffer.len() / CHANNELS) as i64
    }

    fn seek(&mut self, frame: i64) -> Result<(), AudioError> {
        let seconds = frame as f64 / f64::from(RATE) - SEEK_LEAD;
        self.stream.seek(seconds)?;
        self.buffer.clear();
        self.fresh = true;
        self.ended = false;
        Ok(())
    }

    fn decode_more(&mut self) -> Result<(), AudioError> {
        self.native.clear();
        let Some(seconds) = self.stream.next(&mut self.native)? else {
            if let Some(converter) = &mut self.converter {
                converter.finish(&mut self.buffer)?;
            }
            self.ended = true;
            return Ok(());
        };
        if self.fresh {
            self.position = match &mut self.converter {
                Some(converter) => {
                    converter.start((seconds * f64::from(self.stream.rate())).round() as i64)
                }
                None => (seconds * f64::from(RATE)).round() as i64,
            };
            self.fresh = false;
        }
        match &mut self.converter {
            Some(converter) => converter.push(&self.native, &mut self.buffer),
            None => {
                self.buffer.extend_from_slice(&self.native);
                Ok(())
            }
        }
    }

    fn discard_before(&mut self, frame: i64) {
        let drop = (frame - self.position).clamp(0, (self.buffer.len() / CHANNELS) as i64);
        self.buffer.drain(..drop as usize * CHANNELS);
        self.position += drop;
    }
}
