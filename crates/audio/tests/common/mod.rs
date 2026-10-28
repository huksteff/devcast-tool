//! Helpers of the audio tests: synthetic files made by the ffmpeg of `.deps`, whole-track reads
//! and simple measurements.

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "helpers shared by test files, each using its own part; a failed step must fail the test"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use dc_audio::{CHANNELS, RATE, SourceTrack, TrackReader};

pub fn ffmpeg(args: &[&str]) {
    let tool = PathBuf::from(env!("FFMPEG_DIR"))
        .join("bin")
        .join("ffmpeg.exe");
    let output = Command::new(tool)
        .args(["-v", "error", "-y"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "ffmpeg {args:?}: {output:?}");
}

/// Makes `name` in `dir` from the lavfi source `source` with extra output `options`.
pub fn make(dir: &Path, name: &str, source: &str, options: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let mut args = vec!["-f", "lavfi", "-i", source];
    args.extend_from_slice(options);
    args.push(path.to_str().unwrap());
    ffmpeg(&args);
    path
}

pub fn track(path: &Path) -> SourceTrack {
    SourceTrack {
        name: "test".to_owned(),
        path: path.to_path_buf(),
        stream: 0,
    }
}

/// Frames `start..start + frames` of `track`, read in chunks of `chunk` frames.
pub fn read(track: &SourceTrack, start: i64, frames: usize, chunk: usize) -> Vec<f32> {
    let mut reader = TrackReader::open(track).unwrap();
    let mut out = vec![0.0; frames * CHANNELS];
    for (n, part) in out.chunks_mut(chunk * CHANNELS).enumerate() {
        reader.fill(start + (n * chunk) as i64, part).unwrap();
    }
    out
}

/// One channel of interleaved stereo.
pub fn channel(samples: &[f32], index: usize) -> Vec<f32> {
    samples
        .iter()
        .skip(index)
        .step_by(CHANNELS)
        .copied()
        .collect()
}

/// Frequency of a tone from its rising zero crossings, Hz.
pub fn frequency(mono: &[f32]) -> f64 {
    let rising: Vec<usize> = (1..mono.len())
        .filter(|&i| mono[i - 1] < 0.0 && mono[i] >= 0.0)
        .collect();
    let (first, last) = (rising[0], rising[rising.len() - 1]);
    (rising.len() - 1) as f64 * f64::from(RATE) / (last - first) as f64
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, s| m.max(s.abs()))
}

pub fn rms(samples: &[f32]) -> f64 {
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len() as f64).sqrt()
}

/// First frame of interleaved stereo louder than `threshold`.
pub fn onset(samples: &[f32], threshold: f32) -> usize {
    samples.iter().position(|s| s.abs() > threshold).unwrap() / CHANNELS
}
