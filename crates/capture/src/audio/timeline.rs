//! Sound on the host clock (decision 008): a track runs from its start to the stop, sample `n`
//! of it plays at `start + n / RATE` on the host clock, whatever the device did.
//!
//! - A packet is placed by the host time of its first sample. One that is off by more than
//!   [`GAP_SECONDS`] is a gap (loopback sends nothing while the system is silent, a device
//!   changed): the hole is filled with silence, or the overlapping part is cut.
//! - Smaller offsets come from the device clock running at its own speed (~41 ppm on the
//!   output of decision 008); [`DriftLoop`] pulls them in by resampling slightly faster or
//!   slower.
//! - The same resampler converts the device rate to [`RATE`].

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Adjustable, Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use super::device::CHANNELS;
use super::drift::{DriftLoop, MAX_CORRECTION};
use crate::error::CaptureError;
use crate::event::HostTime;

/// Sample rate of every recorded track.
pub const RATE: u32 = 48_000;

/// A packet further than this from its place is a gap or an overlap. Packets of a working
/// device are within ~1 ms of their place (decision 008), drift adds ~0.04 ms per second.
pub const GAP_SECONDS: f64 = 0.005;

/// Device frames per resampler call.
const CHUNK: usize = 1024;

/// The newest frames are held back this long (frames), so that packets which were still on
/// their way when the recording stopped can be cut at the stop. Longer than any packet.
const HOLD: usize = RATE as usize / 10;

/// What the timeline did to the device's sound.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TimelineStats {
    /// Holes filled with silence, start and end included.
    pub gaps: u32,
    /// Seconds of silence filled in.
    pub silence: f64,
    /// Seconds of device sound cut (before the start, overlaps).
    pub cut: f64,
    /// Current speed correction, ppm: minus the device clock's drift against the host clock.
    pub correction_ppm: f64,
    /// The largest offset of a packet from its place that was not a gap, seconds.
    pub worst_offset: f64,
}

pub struct AudioTimeline {
    start: HostTime,
    input_rate: u32,
    resampler: Async<f32>,
    /// Startup output frames of the current resampler, dropped.
    delay: i64,
    /// Device frames (stereo, interleaved) waiting for a full chunk.
    pending: Vec<f32>,
    scratch: Vec<f32>,
    /// Track position where the current resampler starts.
    base: i64,
    /// Output frames of the current resampler, startup included.
    produced: i64,
    /// Frames made: the track position of the next one.
    emitted: i64,
    /// The newest made frames, not handed out yet (see [`HOLD`]).
    held: Vec<f32>,
    /// Where the track ends, once the recording stopped.
    end: Option<i64>,
    /// The current resampler has placed a packet.
    placed: bool,
    relative: f64,
    drift: DriftLoop,
    stats: TimelineStats,
}

impl AudioTimeline {
    pub fn new(start: HostTime, input_rate: u32) -> Result<Self, CaptureError> {
        let resampler = resampler(input_rate)?;
        Ok(Self {
            start,
            input_rate,
            delay: resampler.output_delay() as i64,
            resampler,
            pending: Vec::with_capacity(CHUNK * CHANNELS),
            scratch: Vec::new(),
            base: 0,
            produced: 0,
            emitted: 0,
            held: Vec::with_capacity(2 * HOLD * CHANNELS),
            end: None,
            placed: false,
            relative: 1.0,
            drift: DriftLoop::default(),
            stats: TimelineStats::default(),
        })
    }

    pub fn stats(&self) -> TimelineStats {
        TimelineStats {
            correction_ppm: self.drift.correction_ppm(),
            worst_offset: self.drift.worst(),
            ..self.stats
        }
    }

    /// Frames made so far; after [`Self::finish`] the length of the track.
    pub fn frames(&self) -> i64 {
        self.emitted
    }

    /// Places stereo `samples` whose first frame was captured at `time`; finished track frames
    /// are appended to `out`.
    pub fn push(
        &mut self,
        time: HostTime,
        samples: &[f32],
        out: &mut Vec<f32>,
    ) -> Result<(), CaptureError> {
        let ratio = self.ratio();
        let error = self.position() - self.place(time);
        let mut samples = samples;
        if !self.placed || error.abs() > GAP_SECONDS * f64::from(RATE) {
            if error < 0.0 {
                let frames = (-error / ratio).round() as usize;
                self.stats.gaps += 1;
                self.stats.silence += frames as f64 / f64::from(self.input_rate);
                self.silence(frames, out)?;
            } else {
                let frames = ((error / ratio).round() as usize).min(samples.len() / CHANNELS);
                self.stats.cut += frames as f64 / f64::from(self.input_rate);
                samples = &samples[frames * CHANNELS..];
            }
            self.placed |= !samples.is_empty();
        } else {
            let seconds = (samples.len() / CHANNELS) as f64 / f64::from(self.input_rate);
            self.relative = self.drift.update(error / f64::from(RATE), seconds);
            self.resampler
                .set_resample_ratio_relative(self.relative, true)
                .map_err(|e| CaptureError::audio("подстроить частоту звука", e))?;
        }
        self.feed(samples, out)
    }

    /// The device changed: sound continues from the next packet, at `input_rate`. What the old
    /// resampler still held becomes part of the gap.
    pub fn restart(&mut self, input_rate: u32) -> Result<(), CaptureError> {
        self.resampler = resampler(input_rate)?;
        self.delay = self.resampler.output_delay() as i64;
        self.input_rate = input_rate;
        self.base = self.emitted;
        self.produced = 0;
        self.pending.clear();
        self.placed = false;
        self.relative = 1.0;
        self.drift.restart();
        Ok(())
    }

    /// The recording stopped at `time`: nothing after it is handed out.
    pub fn stop_at(&mut self, time: HostTime) {
        self.end = Some(self.place(time).round() as i64);
    }

    /// Fills silence up to the stop (see [`Self::stop_at`]), cuts what was made past it and
    /// hands out the rest.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), CaptureError> {
        let end = self.end.unwrap_or(self.emitted);
        let missing = end as f64 - self.position();
        if missing > GAP_SECONDS * f64::from(RATE) {
            self.stats.gaps += 1;
            self.stats.silence += missing / f64::from(RATE);
        }
        // Zeros also push out what the resampler still holds.
        while self.emitted < end {
            self.silence(CHUNK, out)?;
        }
        let keep = end - (self.emitted - (self.held.len() / CHANNELS) as i64);
        let keep = usize::try_from(keep).map_err(|_| {
            CaptureError::audio(
                "закончить дорожку звука",
                "звук пришёл позже стопа больше чем на 0,1 с",
            )
        })?;
        self.held.truncate(keep * CHANNELS);
        self.emitted = end;
        out.append(&mut self.held);
        Ok(())
    }

    fn ratio(&self) -> f64 {
        f64::from(RATE) / f64::from(self.input_rate) * self.relative
    }

    /// Track position (fractional frames) where the next device frame lands.
    fn position(&self) -> f64 {
        let pending = (self.pending.len() / CHANNELS) as f64;
        (self.base + self.produced) as f64 + pending * self.ratio()
    }

    /// Track position of host time `time`.
    fn place(&self, time: HostTime) -> f64 {
        (time.micros() - self.start.micros()) as f64 * f64::from(RATE) / 1e6
    }

    fn silence(&mut self, frames: usize, out: &mut Vec<f32>) -> Result<(), CaptureError> {
        let zeros = [0.0; CHUNK * CHANNELS];
        let mut left = frames;
        while left > 0 {
            let now = left.min(CHUNK);
            self.feed(&zeros[..now * CHANNELS], out)?;
            left -= now;
        }
        Ok(())
    }

    fn feed(&mut self, mut samples: &[f32], out: &mut Vec<f32>) -> Result<(), CaptureError> {
        while !samples.is_empty() {
            let free = CHUNK * CHANNELS - self.pending.len();
            let (now, later) = samples.split_at(free.min(samples.len()));
            self.pending.extend_from_slice(now);
            samples = later;
            if self.pending.len() == CHUNK * CHANNELS {
                self.process(out)?;
            }
        }
        Ok(())
    }

    fn process(&mut self, out: &mut Vec<f32>) -> Result<(), CaptureError> {
        let failed =
            |e: &dyn std::fmt::Display| CaptureError::audio("пересчитать частоту звука", e);
        let frames = self.resampler.output_frames_next();
        self.scratch.resize(frames * CHANNELS, 0.0);
        let input =
            InterleavedSlice::new(&self.pending, CHANNELS, CHUNK).map_err(|e| failed(&e))?;
        let mut output = InterleavedSlice::new_mut(&mut self.scratch, CHANNELS, frames)
            .map_err(|e| failed(&e))?;
        let (_, made) = self
            .resampler
            .process_into_buffer(&input, &mut output, None)
            .map_err(|e| failed(&e))?;
        self.pending.clear();
        let first = self.produced;
        self.produced += made as i64;
        let skip = (self.delay - first).clamp(0, made as i64);
        let mut take = made as i64 - skip;
        if let Some(end) = self.end {
            take = take.min(end - self.emitted).max(0);
        }
        let from = skip as usize * CHANNELS;
        self.held
            .extend_from_slice(&self.scratch[from..from + take as usize * CHANNELS]);
        self.emitted += take;
        let old = (self.held.len() / CHANNELS).saturating_sub(HOLD);
        out.extend(self.held.drain(..old * CHANNELS));
        Ok(())
    }
}

/// Converts `input_rate` to [`RATE`], allowing [`MAX_CORRECTION`] of speed change.
fn resampler(input_rate: u32) -> Result<Async<f32>, CaptureError> {
    let mut parameters = SincInterpolationParameters::new(64, WindowFunction::BlackmanHarris2);
    parameters.interpolation = SincInterpolationType::Linear;
    parameters.oversampling_factor = 256;
    Async::new_sinc(
        f64::from(RATE) / f64::from(input_rate),
        1.0 + 2.0 * MAX_CORRECTION,
        &parameters,
        CHUNK,
        CHANNELS,
        FixedAsync::Input,
    )
    .map_err(|e| CaptureError::audio("подготовить пересчёт частоты звука", e))
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
