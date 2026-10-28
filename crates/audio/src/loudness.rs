//! EBU R128 loudness of sound tracks (ebur128), for normalization (decision 000: −16 LUFS,
//! true peak −1 dBTP).
//!
//! A normalized channel is brought to [`TARGET`] by one factor found on its whole source track,
//! before its volume and gain: cutting does not change it, and the track is measured once
//! ([`LoudnessCache`]). The true peak is held by the limiter at the end of the mix. When the
//! factor would push peaks over it (speech often has peaks 20–30 dB above its loudness), the
//! limiter takes some loudness away, so the factor is solved for the limited track to measure
//! [`TARGET`].

use std::collections::HashMap;

use ebur128::{EbuR128, Mode};

use crate::error::AudioError;
use crate::limiter::{CEILING, Limited};
use crate::reader::TrackReader;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE};

/// Integrated loudness of a normalized channel, LUFS.
pub const TARGET: f64 = -16.0;

/// Frames measured at a time.
const BLOCK: usize = RATE as usize;

/// The factor is solved until the limited track is this close to [`TARGET`], LU.
const TOLERANCE: f64 = 0.05;

/// Most passes over a limited track while solving the factor; the secant method needs 2–4.
const MAX_PASSES: usize = 8;

/// Loudness of the output or of a track as EBU R128 measures it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// Integrated loudness, LUFS; `-inf` when everything is below the gate (silence).
    pub integrated: f64,
    /// The highest true peak of the channels, dBTP.
    pub true_peak: f64,
}

/// Measures interleaved stereo at [`RATE`] fed in parts.
pub struct Meter {
    meter: EbuR128,
}

impl Meter {
    pub fn new() -> Result<Self, AudioError> {
        let meter = EbuR128::new(CHANNELS as u32, RATE, Mode::I | Mode::TRUE_PEAK)
            .map_err(AudioError::Loudness)?;
        Ok(Self { meter })
    }

    pub fn add(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        self.meter
            .add_frames_f32(samples)
            .map_err(AudioError::Loudness)
    }

    pub fn result(&self) -> Result<Measurement, AudioError> {
        let integrated = self.meter.loudness_global().map_err(AudioError::Loudness)?;
        let mut peak = 0.0_f64;
        for channel in 0..CHANNELS as u32 {
            let channel_peak = self.meter.true_peak(channel);
            peak = peak.max(channel_peak.map_err(AudioError::Loudness)?);
        }
        Ok(Measurement {
            integrated,
            true_peak: 20.0 * peak.log10(),
        })
    }
}

/// Measurements and normalization factors of whole tracks, each found once.
#[derive(Default)]
pub struct LoudnessCache {
    measured: HashMap<SourceTrack, (Measurement, i64)>,
    factors: HashMap<SourceTrack, f32>,
}

impl LoudnessCache {
    pub fn measure(&mut self, track: &SourceTrack) -> Result<Measurement, AudioError> {
        Ok(self.measured_with_length(track)?.0)
    }

    /// The factor that brings `track` to [`TARGET`] after the limiter; 1 for a track that is
    /// all silence.
    pub fn normalization(&mut self, track: &SourceTrack) -> Result<f32, AudioError> {
        if let Some(factor) = self.factors.get(track) {
            return Ok(*factor);
        }
        let (plain, frames) = self.measured_with_length(track)?;
        if !plain.integrated.is_finite() {
            return Ok(1.0);
        }
        let mut gain = TARGET - plain.integrated;
        if plain.true_peak + gain > 20.0 * f64::from(CEILING).log10() {
            gain = solve_limited(track, frames, gain)?;
        }
        let factor = 10f64.powf(gain / 20.0) as f32;
        self.factors.insert(track.clone(), factor);
        Ok(factor)
    }

    fn measured_with_length(
        &mut self,
        track: &SourceTrack,
    ) -> Result<(Measurement, i64), AudioError> {
        if let Some(found) = self.measured.get(track) {
            return Ok(*found);
        }
        let found = measure_track(track)?;
        self.measured.insert(track.clone(), found);
        Ok(found)
    }
}

/// The gain, dB, at which `track` limited measures [`TARGET`], by the secant method from
/// `gain` (where it measures less: the limiter only takes loudness away).
fn solve_limited(track: &SourceTrack, frames: i64, gain: f64) -> Result<f64, AudioError> {
    let mut a = (gain, limited_loudness(track, frames, gain)?);
    let mut b = (a.0 + (TARGET - a.1), 0.0);
    b.1 = limited_loudness(track, frames, b.0)?;
    for _ in 2..MAX_PASSES {
        if (TARGET - b.1).abs() <= TOLERANCE || b.1 == a.1 {
            break;
        }
        let next = b.0 + (TARGET - b.1) * (b.0 - a.0) / (b.1 - a.1);
        a = b;
        b = (next, limited_loudness(track, frames, next)?);
    }
    Ok(b.0)
}

fn limited_loudness(track: &SourceTrack, frames: i64, gain: f64) -> Result<f64, AudioError> {
    let factor = 10f64.powf(gain / 20.0) as f32;
    let mut reader = TrackReader::open(track)?;
    let mut limited = Limited::default();
    let mut meter = Meter::new()?;
    let mut block = vec![0.0; BLOCK * CHANNELS];
    for start in (0..frames).step_by(BLOCK) {
        let len = (frames - start).min(BLOCK as i64) as usize * CHANNELS;
        limited.render(start, &mut block[..len], |from, buffer| {
            reader.fill(from, buffer)?;
            buffer.iter_mut().for_each(|s| *s *= factor);
            Ok(())
        })?;
        meter.add(&block[..len])?;
    }
    Ok(meter.result()?.integrated)
}

fn measure_track(track: &SourceTrack) -> Result<(Measurement, i64), AudioError> {
    let mut reader = TrackReader::open(track)?;
    let mut meter = Meter::new()?;
    let mut block = vec![0.0; BLOCK * CHANNELS];
    let mut start = 0;
    while let Some(frames) = reader.fill_until_end(start, &mut block)? {
        meter.add(&block[..frames * CHANNELS])?;
        start += frames as i64;
    }
    Ok((meter.result()?, start))
}
