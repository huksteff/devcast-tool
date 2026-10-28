//! The limiter at the end of the mix: no sample and no peak between samples goes above
//! [`CEILING`] (−1 dBTP, decision 000), so a gain up to 10× never clips.
//!
//! The gain at each frame is a pure function of the sound around it, not of what was rendered
//! before, so preview after a jump and export give the same result:
//! - the needed gain of a frame is `CEILING / true peak` (4× oversampled like EBU R128),
//!   held over the oversampling filter, so the whole neighbourhood of a peak is scaled alike;
//! - it falls linearly over [`ATTACK`] before a peak and recovers linearly over [`RELEASE`].
//!
//! [`Limited`] runs it over a stream: it asks its source for the context ([`BEFORE`] frames
//! before the asked ones and [`AFTER`] after) and keeps it for the next part in order.

use crate::error::AudioError;
use crate::{CHANNELS, RATE};

/// −1 dBTP.
pub const CEILING: f32 = 0.891_250_9;

const ATTACK: usize = RATE as usize / 200;
const RELEASE: usize = RATE as usize / 10;

/// True peak: 4× oversampling with a 49-tap Hann-windowed sinc, as in libebur128.
const FACTOR: usize = 4;
const TAPS: usize = 49;
/// Delay of the oversampling filter, frames.
const DELAY: usize = (TAPS - 1) / 2 / FACTOR;
/// The filter reaches this far on each side, frames.
const REACH: usize = TAPS / FACTOR + 1;
const HOLD: usize = REACH;

/// Context needed before and after the limited frames.
const BEFORE: usize = RELEASE + HOLD + REACH;
const AFTER: usize = ATTACK + HOLD + REACH;

/// The largest gain that keeps `peak` at or below [`CEILING`] after `f32` rounding.
fn needed_gain(peak: f32) -> f32 {
    if peak <= CEILING {
        return 1.0;
    }
    let gain = CEILING / peak;
    if peak * gain > CEILING {
        gain.next_down()
    } else {
        gain
    }
}

/// The limiter over a stream of sound from a source.
#[derive(Default)]
pub(crate) struct Limited {
    limiter: Limiter,
    /// The source from frame `input_start`: the context kept for the next part.
    input: Vec<f32>,
    input_start: i64,
}

impl Limited {
    /// Fills `out` with frames from `start` of `source(first frame, buffer)`, limited.
    pub fn render(
        &mut self,
        start: i64,
        out: &mut [f32],
        mut source: impl FnMut(i64, &mut [f32]) -> Result<(), AudioError>,
    ) -> Result<(), AudioError> {
        let from = start - BEFORE as i64;
        let to = start + (out.len() / CHANNELS + AFTER) as i64;
        let input_end = self.input_start + (self.input.len() / CHANNELS) as i64;
        if (self.input_start..=input_end).contains(&from) {
            let passed = (from - self.input_start) as usize;
            self.input.drain(..passed * CHANNELS);
            self.limiter.advance(passed);
        } else {
            self.input.clear();
            self.limiter.reset();
        }
        self.input_start = from;
        let have = from + (self.input.len() / CHANNELS) as i64;
        if have < to {
            let old = self.input.len();
            self.input.resize((to - from) as usize * CHANNELS, 0.0);
            if let Err(error) = source(have, &mut self.input[old..]) {
                self.input.clear();
                self.limiter.reset();
                return Err(error);
            }
        }
        let window = (to - from) as usize * CHANNELS;
        self.limiter.apply(&self.input[..window], out);
        Ok(())
    }
}

struct Limiter {
    /// Taps of the phases between samples (1/4, 2/4, 3/4).
    phases: [Vec<f32>; FACTOR - 1],
    peak: Vec<f32>,
    gain: Vec<f32>,
    held: Vec<f32>,
}

impl Default for Limiter {
    fn default() -> Self {
        let tap = |j: usize| {
            let x = (j as f64 - (TAPS / 2) as f64) / FACTOR as f64;
            let sinc = if x == 0.0 {
                1.0
            } else {
                (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
            };
            let window =
                0.5 - 0.5 * (2.0 * std::f64::consts::PI * j as f64 / (TAPS - 1) as f64).cos();
            (sinc * window) as f32
        };
        let phase = |p: usize| (p..TAPS).step_by(FACTOR).map(tap).collect();
        Self {
            phases: [phase(1), phase(2), phase(3)],
            peak: Vec::new(),
            gain: Vec::new(),
            held: Vec::new(),
        }
    }
}

impl Limiter {
    /// Forgets the first `frames` frames of the input: the next input starts that much later.
    fn advance(&mut self, frames: usize) {
        self.peak.drain(..frames.min(self.peak.len()));
    }

    /// Forgets the input: the next one is somewhere else.
    fn reset(&mut self) {
        self.peak.clear();
    }

    /// Limits `input` (interleaved, [`BEFORE`] + `out` + [`AFTER`] frames) into `out`. The
    /// input starts where the last one did, after [`Self::advance`]; what was seen is kept.
    fn apply(&mut self, input: &[f32], out: &mut [f32]) {
        let frames = input.len() / CHANNELS;
        self.measure(input);
        self.gain.clear();
        self.gain
            .extend(self.peak[..frames].iter().map(|&p| needed_gain(p)));
        self.held.clear();
        self.held.extend((0..frames).map(|k| {
            let window = &self.gain[k.saturating_sub(HOLD)..(k + HOLD + 1).min(frames)];
            window.iter().copied().fold(1.0, f32::min)
        }));
        let mut g = 1.0_f32;
        for held in &mut self.held {
            g = held.min(g + 1.0 / RELEASE as f32).min(1.0);
            *held = g;
        }
        g = 1.0;
        for held in self.held.iter_mut().rev() {
            g = held.min(g + 1.0 / ATTACK as f32);
            *held = g;
        }
        let gains = self.held[BEFORE..].iter();
        let samples = input[BEFORE * CHANNELS..].as_chunks::<CHANNELS>().0;
        let targets = out.as_chunks_mut::<CHANNELS>().0;
        for ((target, frame), gain) in targets.iter_mut().zip(samples).zip(gains) {
            for (o, s) in target.iter_mut().zip(frame) {
                *o = s * gain;
            }
        }
    }

    /// `peak[k]`: the largest magnitude at frame `k` and between it and the next frame, for
    /// the frames of `input` not measured yet.
    fn measure(&mut self, input: &[f32]) {
        let frames = input.len() / CHANNELS;
        let known = self.peak.len();
        if known >= frames {
            return;
        }
        let new = input[known * CHANNELS..].as_chunks::<CHANNELS>().0;
        self.peak.extend(
            new.iter()
                .map(|f| f.iter().fold(0.0_f32, |m, s| m.max(s.abs()))),
        );
        for n in known.max(DELAY)..frames {
            let mut top = 0.0_f32;
            for taps in &self.phases {
                for channel in 0..CHANNELS {
                    let value: f32 = taps
                        .iter()
                        .zip((0..=n).rev())
                        .map(|(h, i)| h * input[i * CHANNELS + channel])
                        .sum();
                    top = top.max(value.abs());
                }
            }
            for at in n - DELAY..=n - DELAY + 1 {
                self.peak[at] = self.peak[at].max(top);
            }
        }
    }
}
