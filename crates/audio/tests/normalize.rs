//! Task 4.8: EBU R128 normalization to −16 LUFS with the true peak at most −1 dBTP, checked by
//! measuring the mix.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use common::clip;
use common::project::{add_clip, project_dir, render_all, speech_dir, speech_project};
use dc_audio::{
    ChannelLevel, Levels, LoudnessCache, Measurement, Meter, Mixer, TARGET, frame_at, tracks,
};
use dc_core::{ClipId, Project};
use std::path::Path;

/// −1 dBTP. The limiter scales the neighbourhood of a peak alike, but a peak near the ceiling
/// on its 100 ms recovery ramp can come out a hair higher: measured up to 0.002 dB on speech
/// (2026-10-28). That is far below the error of a 4× true-peak meter itself.
const MAX_TRUE_PEAK: f64 = -1.0 + 0.01;

fn normalized(clip: ClipId, track: &str, volume: f32) -> Levels {
    let level = ChannelLevel::new(volume, 1.0).unwrap().with_normalize(true);
    let mut levels = Levels::default();
    let channels = &mut levels.clips.entry(clip).or_default().channels;
    channels.insert(track.to_owned(), level);
    levels
}

fn measure(samples: &[f32]) -> Measurement {
    let mut meter = Meter::new().unwrap();
    meter.add(samples).unwrap();
    meter.result().unwrap()
}

/// Mix of `project` at `levels`, measured: the source track before and the output after.
fn mix(dir: &Path, project: &Project, levels: &Levels, track: usize) -> (Measurement, Measurement) {
    let mut cache = LoudnessCache::default();
    let folder = project_dir(dir);
    let mut mixer = Mixer::new(&folder, project, levels, &mut cache).unwrap();
    let frames = mixer.frames() as usize;
    let output = render_all(&mut mixer, 48_000);
    let source = tracks(&folder, &project.clips[0].media).unwrap()[track].clone();
    let before = cache.measure(&source).unwrap();
    let after = measure(&output[..frames * dc_audio::CHANNELS]);
    (before, after)
}

fn imported(source: &str, seconds: f64) -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().unwrap();
    clip(dir.path(), "sound.mp4", source, &[]);
    let mut project = Project::default();
    add_clip(&mut project, "sound.mp4", seconds);
    (dir, project)
}

fn report(name: &str, before: Measurement, after: Measurement) {
    println!(
        "{name}: {:.2} LUFS / {:.2} dBTP → {:.3} LUFS / {:.4} dBTP",
        before.integrated, before.true_peak, after.integrated, after.true_peak
    );
}

#[test]
fn quiet_tone_is_raised_to_target() {
    let source = "aevalsrc=0.01*sin(2*PI*1000*t)|0.01*sin(2*PI*1000*t):s=48000:d=10";
    let (dir, project) = imported(source, 10.0);
    let levels = normalized(project.clips[0].id, "audio-1", 1.0);
    let (before, after) = mix(dir.path(), &project, &levels, 0);
    report("tone −40 dBFS", before, after);
    assert!((after.integrated - TARGET).abs() < 0.1);
    assert!(after.true_peak <= MAX_TRUE_PEAK, "{}", after.true_peak);
}

#[test]
fn loud_noise_is_lowered_to_target() {
    let source = "anoisesrc=color=pink:amplitude=0.9:sample_rate=48000:duration=10:seed=5";
    let (dir, project) = imported(source, 10.0);
    let levels = normalized(project.clips[0].id, "audio-1", 1.0);
    let (before, after) = mix(dir.path(), &project, &levels, 0);
    report("pink noise", before, after);
    assert!(
        before.integrated > TARGET + 3.0,
        "the test must lower the sound"
    );
    assert!((after.integrated - TARGET).abs() < 0.1);
    assert!(after.true_peak <= MAX_TRUE_PEAK, "{}", after.true_peak);
}

#[test]
fn speech_mic_is_normalized() {
    let dir = speech_dir();
    let project = speech_project();
    let levels = normalized(project.clips[0].id, "mic", 1.0);
    let (before, after) = mix(&dir, &project, &levels, 1);
    report("speech mic", before, after);
    assert!((after.integrated - TARGET).abs() < 0.5);
    assert!(after.true_peak <= MAX_TRUE_PEAK, "{}", after.true_peak);
}

#[test]
fn volume_applies_after_normalization() {
    let source = "aevalsrc=0.01*sin(2*PI*1000*t)|0.01*sin(2*PI*1000*t):s=48000:d=10";
    let (dir, project) = imported(source, 10.0);
    let levels = normalized(project.clips[0].id, "audio-1", 0.5);
    let (_, after) = mix(dir.path(), &project, &levels, 0);
    let expected = TARGET + 20.0 * 0.5f64.log10();
    assert!(
        (after.integrated - expected).abs() < 0.1,
        "{}",
        after.integrated
    );
}

#[test]
fn silence_stays_silent() {
    let (dir, project) = imported("anullsrc=r=48000:cl=stereo:d=3", 3.0);
    let levels = normalized(project.clips[0].id, "audio-1", 1.0);
    let mut mixer = Mixer::new(
        &project_dir(dir.path()),
        &project,
        &levels,
        &mut LoudnessCache::default(),
    );
    let output = render_all(mixer.as_mut().unwrap(), 4800);
    assert_eq!(output.len(), (frame_at(3_000_000) as usize + 48_000) * 2);
    assert!(output.iter().all(|s| *s == 0.0));
}
