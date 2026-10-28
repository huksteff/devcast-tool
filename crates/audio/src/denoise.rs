//! Noise suppression of a track (task 4.9): RNNoise (nnnoiseless) on each channel, as a filter
//! while mixing; the file is not changed. RNNoise is a recurrent network that learns the noise
//! as it goes, so it cannot jump: after a seek it starts again [`WARMUP`] before the asked
//! frame and drops that. Its output is late by [`LATENCY`] frames, read that much ahead so the
//! sound stays in sync with the picture.

use nnnoiseless::DenoiseState;

use crate::error::AudioError;
use crate::reader::TrackReader;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE};

/// Frames RNNoise takes at a time: 10 ms.
const FRAME: usize = DenoiseState::FRAME_SIZE;

/// Frames by which the output of RNNoise is late (its overlap-add window), measured by the test
/// `denoise_tests::output_is_in_sync`.
pub(crate) const LATENCY: i64 = FRAME as i64;

/// Frames processed and dropped before the asked one after a seek: 0.5 s, enough for the
/// network to settle on the noise.
const WARMUP: i64 = RATE as i64 / 2;

/// RNNoise works on 16-bit sample values.
const SCALE: f32 = 32768.0;

/// The sound of a track as the mix reads it: as is, or with noise suppressed.
pub(crate) enum SourceReader {
    Plain(Box<TrackReader>),
    Denoised(Box<Denoiser>),
}

impl SourceReader {
    pub fn open(track: &SourceTrack, denoise: bool) -> Result<Self, AudioError> {
        let reader = TrackReader::open(track)?;
        Ok(if denoise {
            Self::Denoised(Box::new(Denoiser::new(reader)))
        } else {
            Self::Plain(Box::new(reader))
        })
    }

    /// Like [`TrackReader::fill`]: frames from source frame `start`.
    pub fn fill(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        match self {
            Self::Plain(reader) => reader.fill(start, out),
            Self::Denoised(denoiser) => denoiser.fill(start, out),
        }
    }
}

pub(crate) struct Denoiser {
    reader: TrackReader,
    states: Vec<Box<DenoiseState<'static>>>,
    /// Source frame of `ready[0]`; `None` before the first read.
    next: Option<i64>,
    /// Denoised frames, interleaved.
    ready: Vec<f32>,
    input: Vec<f32>,
    channel_in: Vec<f32>,
    channel_out: Vec<f32>,
}

impl Denoiser {
    fn new(reader: TrackReader) -> Self {
        Self {
            reader,
            states: Vec::new(),
            next: None,
            ready: Vec::new(),
            input: vec![0.0; FRAME * CHANNELS],
            channel_in: vec![0.0; FRAME],
            channel_out: vec![0.0; FRAME],
        }
    }

    fn fill(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        let frames = (out.len() / CHANNELS) as i64;
        let continues = self
            .next
            .is_some_and(|next| next <= start && start <= self.end());
        if !continues {
            self.restart(start)?;
        }
        while self.end() < start + frames {
            self.process()?;
        }
        let next = self.next.unwrap_or(start);
        let skip = (start - next) as usize * CHANNELS;
        out.copy_from_slice(&self.ready[skip..skip + out.len()]);
        self.ready.drain(..skip + out.len());
        self.next = Some(start + frames);
        Ok(())
    }

    /// Source frame after the last ready one.
    fn end(&self) -> i64 {
        self.next.unwrap_or(0) + (self.ready.len() / CHANNELS) as i64
    }

    /// New network state; the frames before `start` are processed and dropped.
    fn restart(&mut self, start: i64) -> Result<(), AudioError> {
        self.states = (0..CHANNELS).map(|_| DenoiseState::new()).collect();
        self.next = Some(start - WARMUP);
        self.ready.clear();
        while self.end() <= start {
            self.process()?;
        }
        let drop = (start - self.next.unwrap_or(start)) as usize * CHANNELS;
        self.ready.drain(..drop.min(self.ready.len()));
        self.next = Some(start);
        Ok(())
    }

    /// Feeds the next [`FRAME`] source frames and keeps the denoised result.
    fn process(&mut self) -> Result<(), AudioError> {
        let feed = self.end() + LATENCY;
        self.reader.fill(feed, &mut self.input)?;
        let start = self.ready.len();
        self.ready.resize(start + FRAME * CHANNELS, 0.0);
        for (channel, state) in self.states.iter_mut().enumerate() {
            for (i, sample) in self.channel_in.iter_mut().enumerate() {
                *sample = self.input[i * CHANNELS + channel] * SCALE;
            }
            state.process_frame(&mut self.channel_out, &self.channel_in);
            for (i, sample) in self.channel_out.iter().enumerate() {
                self.ready[start + i * CHANNELS + channel] = sample / SCALE;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "denoise_tests.rs"]
mod tests;
