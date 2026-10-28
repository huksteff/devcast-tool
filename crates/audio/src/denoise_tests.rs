use std::path::{Path, PathBuf};
use std::process::Command;

use dc_project::ProjectDir;

use super::*;
use crate::track::tracks;

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(path)
}

fn track(path: &Path) -> SourceTrack {
    SourceTrack {
        name: "test".to_owned(),
        path: path.to_path_buf(),
        stream: 0,
        origin: 0,
    }
}

/// Clean synthesized Russian speech (`fixtures/audio`) with pink noise of amplitude 0.03 under
/// it, stereo, in `dir`.
fn noisy_speech(dir: &Path) -> PathBuf {
    let path = dir.join("noisy.flac");
    let tool = PathBuf::from(env!("FFMPEG_DIR"))
        .join("bin")
        .join("ffmpeg.exe");
    let mix = "[0]aformat=channel_layouts=stereo[s];[s][1]amix=inputs=2:duration=first:normalize=0";
    let status = Command::new(tool)
        .args(["-v", "error", "-y", "-i"])
        .arg(fixture("audio/tts-ru.flac"))
        .args([
            "-f",
            "lavfi",
            "-i",
            "anoisesrc=color=pink:amplitude=0.03:r=48000:seed=7",
        ])
        .args(["-filter_complex", mix, "-c:a", "flac"])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    path
}

fn read(reader: &mut SourceReader, from: f64, to: f64, chunk: usize) -> Vec<f32> {
    let (start, end) = (
        (from * f64::from(RATE)) as i64,
        (to * f64::from(RATE)) as i64,
    );
    let mut out = vec![0.0; (end - start) as usize * CHANNELS];
    for (n, part) in out.chunks_mut(chunk * CHANNELS).enumerate() {
        reader.fill(start + (n * chunk) as i64, part).unwrap();
    }
    out
}

fn power(samples: &[f32]) -> f64 {
    samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64
}

fn db(power: f64) -> f64 {
    10.0 * power.log10()
}

/// Sums of power over the 100 ms blocks where the clean speech talks, and where it is silent.
fn speech_and_pauses(clean: &[f32], sound: &[f32]) -> (f64, f64) {
    let block = RATE as usize / 10 * CHANNELS;
    let (mut speech, mut pauses) = (0.0, 0.0);
    for (reference, part) in clean.chunks(block).zip(sound.chunks(block)) {
        let reference = power(reference);
        if reference == 0.0 {
            pauses += power(part);
        } else if db(reference) > -40.0 {
            speech += power(part);
        }
    }
    (speech, pauses)
}

#[test]
fn noise_goes_and_speech_stays() {
    let dir = tempfile::tempdir().unwrap();
    let noisy = track(&noisy_speech(dir.path()));
    let clean_speech = track(&fixture("audio/tts-ru.flac"));
    let clean = read(
        &mut SourceReader::open(&clean_speech, false).unwrap(),
        0.5,
        9.5,
        4800,
    );
    let plain = read(
        &mut SourceReader::open(&noisy, false).unwrap(),
        0.5,
        9.5,
        4800,
    );
    let denoised = read(
        &mut SourceReader::open(&noisy, true).unwrap(),
        0.5,
        9.5,
        4800,
    );
    let (before, after) = (
        speech_and_pauses(&clean, &plain),
        speech_and_pauses(&clean, &denoised),
    );
    let (speech, pauses) = (db(after.0) - db(before.0), db(after.1) - db(before.1));
    println!("speech {speech:.1} dB, pauses {pauses:.1} dB");
    assert!(
        speech.abs() < 1.5,
        "speech keeps its loudness: {speech:.1} dB"
    );
    assert!(pauses < -25.0, "noise is suppressed: {pauses:.1} dB");
}

#[test]
fn hiss_of_the_owners_microphone_goes() {
    let dir = ProjectDir::open(fixture("recordings/speech.dcproj")).unwrap();
    let project = dir.load().unwrap();
    let mic = tracks(&dir, &project.clips[0].media)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "mic")
        .unwrap();
    // 1–4 s is a pause (tests/silence.rs): only the hiss of the raised microphone.
    let plain = power(&read(
        &mut SourceReader::open(&mic, false).unwrap(),
        1.0,
        4.0,
        4800,
    ));
    let clean = power(&read(
        &mut SourceReader::open(&mic, true).unwrap(),
        1.0,
        4.0,
        4800,
    ));
    let change = db(clean) - db(plain);
    println!("hiss {change:.1} dB");
    assert!(change < -15.0, "{change:.1} dB");
}

#[test]
fn output_is_in_sync() {
    let dir = tempfile::tempdir().unwrap();
    let noisy = track(&noisy_speech(dir.path()));
    let plain = read(
        &mut SourceReader::open(&noisy, false).unwrap(),
        1.0,
        9.0,
        4800,
    );
    let clean = read(
        &mut SourceReader::open(&noisy, true).unwrap(),
        1.0,
        9.0,
        4800,
    );
    // Loudness envelopes in 1 ms steps: the waveform itself changes, its timing must not.
    let envelope = |s: &[f32]| -> Vec<f64> {
        s.chunks(48 * CHANNELS)
            .map(|c| c.iter().map(|x| f64::from(*x).powi(2)).sum())
            .collect()
    };
    let (plain, clean) = (envelope(&plain), envelope(&clean));
    let correlation = |lag: i64| -> f64 {
        (50..plain.len() - 50)
            .map(|i| plain[i] * clean[(i as i64 + lag) as usize])
            .sum()
    };
    let best = (-30..=30)
        .max_by(|a, b| correlation(*a).total_cmp(&correlation(*b)))
        .unwrap();
    assert_eq!(best, 0, "denoised sound is {best} ms off");
}

#[test]
fn chunk_size_does_not_change_the_sound() {
    let dir = tempfile::tempdir().unwrap();
    let noisy = track(&noisy_speech(dir.path()));
    let whole = read(
        &mut SourceReader::open(&noisy, true).unwrap(),
        1.0,
        4.0,
        144_000,
    );
    let pieces = read(
        &mut SourceReader::open(&noisy, true).unwrap(),
        1.0,
        4.0,
        333,
    );
    assert_eq!(whole, pieces);
}

#[test]
fn a_seek_settles_within_the_warmup() {
    let dir = tempfile::tempdir().unwrap();
    let noisy = track(&noisy_speech(dir.path()));
    let straight = read(
        &mut SourceReader::open(&noisy, true).unwrap(),
        2.0,
        6.0,
        4800,
    );
    let mut jumped = SourceReader::open(&noisy, true).unwrap();
    read(&mut jumped, 8.0, 9.0, 4800);
    let after = read(&mut jumped, 4.0, 6.0, 4800);
    let same = &straight[2 * RATE as usize * CHANNELS..];
    let difference: Vec<f32> = same.iter().zip(&after).map(|(a, b)| a - b).collect();
    let error = db(power(&difference)) - db(power(same));
    println!("after a seek the sound differs by {error:.1} dB relative");
    assert!(error < -20.0, "{error:.1} dB");
}
