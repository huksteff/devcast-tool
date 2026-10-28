//! The sound Whisper hears: tracks of a recording summed to mono and brought from 48 kHz to
//! 16 kHz (low-pass below 8 kHz, then every third sample).

use dc_audio::{CHANNELS, RATE, SourceTrack, TrackReader};

use crate::error::{Result, TranscribeError};
use crate::recognize::WHISPER_RATE;

/// 48 kHz → 16 kHz.
const STEP: usize = (RATE / WHISPER_RATE) as usize;

/// Taps of the low-pass filter (odd, so it has a middle and no delay after centering).
const TAPS: usize = 63;

/// Frames read at a time.
const CHUNK: usize = 48_000;

/// Frames `start..end` (48 kHz) of `tracks` summed, at [`WHISPER_RATE`] mono.
pub fn speech_samples(tracks: &[SourceTrack], start: i64, end: i64) -> Result<Vec<f32>> {
    let audio = |e: dc_audio::AudioError| TranscribeError::Audio(e.to_string());
    let frames = (end - start).max(0) as usize;
    let mut mono = vec![0.0f32; frames];
    let mut buffer = vec![0.0f32; CHUNK * CHANNELS];
    for track in tracks {
        let mut reader = TrackReader::open(track).map_err(audio)?;
        for (n, part) in mono.chunks_mut(CHUNK).enumerate() {
            let chunk = &mut buffer[..part.len() * CHANNELS];
            reader
                .fill(start + (n * CHUNK) as i64, chunk)
                .map_err(audio)?;
            for (out, frame) in part.iter_mut().zip(chunk.as_chunks::<CHANNELS>().0) {
                *out += frame.iter().sum::<f32>() / CHANNELS as f32;
            }
        }
    }
    Ok(decimate(&mono))
}

/// Every [`STEP`]-th sample of `samples` after a windowed-sinc low-pass at 7.2 kHz.
fn decimate(samples: &[f32]) -> Vec<f32> {
    let filter = low_pass();
    let half = TAPS / 2;
    (0..samples.len().div_ceil(STEP))
        .map(|i| {
            let center = i * STEP;
            filter
                .iter()
                .enumerate()
                .filter_map(|(k, tap)| {
                    let at = (center + k).checked_sub(half)?;
                    samples.get(at).map(|s| s * tap)
                })
                .sum()
        })
        .collect()
}

fn low_pass() -> Vec<f32> {
    let cutoff = 7_200.0 / f64::from(RATE);
    let half = (TAPS / 2) as f64;
    let taps: Vec<f64> = (0..TAPS)
        .map(|k| {
            let x = k as f64 - half;
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let window =
                0.54 - 0.46 * (2.0 * std::f64::consts::PI * k as f64 / (TAPS - 1) as f64).cos();
            sinc * window
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    taps.iter().map(|t| (t / sum) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|n| {
                (2.0 * std::f64::consts::PI * frequency * n as f64 / f64::from(RATE)).sin() as f32
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn speech_band_passes_and_aliases_are_cut() {
        let voice = decimate(&tone(1_000.0, 48_000));
        assert_eq!(voice.len(), 16_000);
        assert!((rms(&voice[100..15_900]) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
        let high = decimate(&tone(12_000.0, 48_000));
        assert!(rms(&high[100..15_900]) < 0.01, "12 kHz would fold to 4 kHz");
    }

    #[test]
    fn no_delay() {
        let mut click = vec![0.0; 3000];
        click[1500] = 1.0;
        let out = decimate(&click);
        let peak = (0..out.len())
            .max_by(|a, b| out[*a].total_cmp(&out[*b]))
            .unwrap();
        assert_eq!(peak, 500);
    }
}
