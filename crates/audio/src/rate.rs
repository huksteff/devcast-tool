//! Conversion of stereo interleaved `f32` from a stream's rate to [`RATE`] with rubato (FFT,
//! synchronous, fixed input and output blocks). The resampler's startup delay is dropped, so
//! output frame `k` is at input time `k / RATE`, and the end is flushed: `n` input frames give
//! `ceil(n · RATE / rate)`.

use std::path::{Path, PathBuf};

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::error::AudioError;
use crate::{CHANNELS, RATE};

/// Wanted input frames per resampler call; rubato rounds it up to a whole number of its
/// smallest blocks.
const CHUNK: usize = 1024;

pub(crate) struct RateConverter {
    resampler: Fft<f32>,
    path: PathBuf,
    rate: u32,
    /// Input frames waiting for a full chunk.
    pending: Vec<f32>,
    scratch: Vec<f32>,
    /// Output frames still to drop: the resampler's startup delay.
    delay: usize,
    input_frames: u64,
    output_frames: u64,
}

impl RateConverter {
    pub fn new(rate: u32, path: &Path) -> Result<Self, AudioError> {
        let resampler = Fft::new(
            rate as usize,
            RATE as usize,
            CHUNK,
            CHANNELS,
            FixedSync::Both,
        )
        .map_err(|e| AudioError::resample(path, e))?;
        Ok(Self {
            delay: resampler.output_delay(),
            scratch: vec![0.0; resampler.output_frames_max() * CHANNELS],
            resampler,
            path: path.to_path_buf(),
            rate,
            pending: Vec::new(),
            input_frames: 0,
            output_frames: 0,
        })
    }

    /// Starts a new run whose first input frame is frame `first` of the stream; returns the
    /// output frame of the run's first output.
    ///
    /// Runs start on a grid of whole resampler chunks (silence is put before `first`): the
    /// resampler then cuts the stream into the same blocks wherever it was started, so a read
    /// after a seek gives the same samples as reading through.
    pub fn start(&mut self, first: i64) -> i64 {
        let chunk = self.resampler.input_frames_next() as i64;
        let aligned = first.div_euclid(chunk) * chunk;
        self.resampler.reset();
        self.delay = self.resampler.output_delay();
        self.pending.clear();
        self.pending
            .resize((first - aligned) as usize * CHANNELS, 0.0);
        self.input_frames = (first - aligned) as u64;
        self.output_frames = 0;
        aligned * i64::from(RATE) / i64::from(self.rate)
    }

    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), AudioError> {
        self.pending.extend_from_slice(input);
        self.input_frames += (input.len() / CHANNELS) as u64;
        let mut used = 0;
        while self.pending.len() - used >= self.resampler.input_frames_next() * CHANNELS {
            used += self.process(used, None, out)?;
        }
        self.pending.drain(..used);
        Ok(())
    }

    /// Converts the rest after the last input and appends it to `out`.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), AudioError> {
        let expected = (self.input_frames * u64::from(RATE)).div_ceil(u64::from(self.rate));
        while self.output_frames < expected {
            let partial = self.pending.len() / CHANNELS;
            self.process(0, Some(partial), out)?;
            self.pending.clear();
        }
        let extra = (self.output_frames - expected) as usize;
        out.truncate(out.len() - extra * CHANNELS);
        self.output_frames = expected;
        Ok(())
    }

    /// One resampler call from `pending[offset..]`; returns the input samples used.
    fn process(
        &mut self,
        offset: usize,
        partial: Option<usize>,
        out: &mut Vec<f32>,
    ) -> Result<usize, AudioError> {
        let frames_in = self.resampler.input_frames_next();
        let mut input_buffer = std::mem::take(&mut self.pending);
        input_buffer.resize(input_buffer.len().max(offset + frames_in * CHANNELS), 0.0);
        let input = InterleavedSlice::new(&input_buffer[offset..], CHANNELS, frames_in)
            .map_err(|e| AudioError::resample(&self.path, e))?;
        let frames_out = self.resampler.output_frames_next();
        let mut output = InterleavedSlice::new_mut(&mut self.scratch, CHANNELS, frames_out)
            .map_err(|e| AudioError::resample(&self.path, e))?;
        let indexing = partial.map(|len| Indexing::new().partial_len(len));
        let (used, made) = self
            .resampler
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(|e| AudioError::resample(&self.path, e))?;
        self.pending = input_buffer;
        let dropped = made.min(self.delay);
        self.delay -= dropped;
        out.extend_from_slice(&self.scratch[dropped * CHANNELS..made * CHANNELS]);
        self.output_frames += (made - dropped) as u64;
        Ok(used * CHANNELS)
    }
}
