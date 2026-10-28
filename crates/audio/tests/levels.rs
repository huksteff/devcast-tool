//! Task 4.7: volume, gain up to 10× without clipping, mute of a channel and of a clip.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use common::project::{
    add_clip, mixer, project_dir, range, render_all, speech_dir, speech_project,
};
use common::{channel, clip, peak, read, rms, track};
use dc_audio::{
    CEILING, CHANNELS, ChannelLevel, ClipLevels, GAIN_MAX, Levels, RATE, frame_at, tracks,
};
use dc_core::{ClipId, Project};

const QUIET: &str = "aevalsrc=0.05*sin(2*PI*440*t)|0.05*sin(2*PI*440*t):s=48000:d=2";

fn level(volume: f32, gain: f32, muted: bool) -> ChannelLevel {
    ChannelLevel::new(volume, gain).unwrap().with_mute(muted)
}

fn clip_levels(clip: ClipId, track: &str, channel: ChannelLevel) -> Levels {
    let mut levels = Levels::default();
    let clip_levels = levels.clips.entry(clip).or_default();
    clip_levels.channels.insert(track.to_owned(), channel);
    levels
}

/// A project of the quiet tone and its sound as read straight from the file.
fn quiet() -> (tempfile::TempDir, Project, Vec<f32>) {
    let dir = tempfile::tempdir().unwrap();
    let path = clip(dir.path(), "tone.mp4", QUIET, &[]);
    let mut project = Project::default();
    add_clip(&mut project, "tone.mp4", 2.0);
    let source = read(&track(&path), 0, 3 * RATE as usize, 4096);
    (dir, project, source)
}

fn worst_ratio(mixed: &[f32], source: &[f32], factor: f32) -> f32 {
    mixed
        .iter()
        .zip(source)
        .fold(0.0, |m, (a, b)| m.max((a - b * factor).abs()))
}

#[test]
fn volume_and_gain_scale_the_track() {
    let (dir, project, source) = quiet();
    let clip = project.clips[0].id;
    for (volume, gain) in [(0.5, 1.0), (1.0, 10.0), (0.3, 4.0), (0.0, 10.0)] {
        let levels = clip_levels(clip, "audio-1", level(volume, gain, false));
        let mixed = render_all(&mut mixer(dir.path(), &project, &levels), 4096);
        let worst = worst_ratio(&mixed, &source, volume * gain);
        assert!(worst < 1e-6, "volume {volume}, gain {gain}: off by {worst}");
    }
}

#[test]
fn limits_are_checked() {
    assert!(ChannelLevel::new(1.01, 1.0).is_err());
    assert!(ChannelLevel::new(-0.1, 1.0).is_err());
    assert!(ChannelLevel::new(1.0, 0.9).is_err());
    assert!(ChannelLevel::new(1.0, GAIN_MAX + 0.01).is_err());
    assert!(ChannelLevel::new(0.0, GAIN_MAX).is_ok());
}

#[test]
fn gain_ten_on_speech_never_clips() {
    let dir = speech_dir();
    let project = speech_project();
    let mic = level(1.0, GAIN_MAX, false);
    let levels = clip_levels(project.clips[0].id, "mic", mic);
    let mut mixer = mixer(&dir, &project, &levels);
    let mixed = render_all(&mut mixer, 4096);
    let found = tracks(&project_dir(&dir), &project.clips[0].media).unwrap();
    let frames = mixer.frames() as usize;
    let mic = read(&found[1], 0, frames, 48_000);
    let system = read(&found[0], 0, frames, 48_000);
    let boosted: Vec<f32> = mic
        .iter()
        .zip(&system)
        .map(|(m, s)| m * GAIN_MAX + s)
        .collect();
    let limited = boosted
        .iter()
        .zip(&mixed)
        .filter(|(b, m)| (*b - *m).abs() > 1e-6);
    let share = limited.count() as f64 / boosted.len() as f64;
    println!(
        "speech mic ×10: peak without limiter {:.2}, with {:.4} (ceiling {CEILING}); rms {:.4} → {:.4}; {:.1}% of samples touched",
        peak(&boosted),
        peak(&mixed),
        rms(&boosted),
        rms(&mixed),
        share * 100.0
    );
    assert!(peak(&boosted) > 1.0, "the test must drive the limiter");
    assert!(peak(&mixed) <= CEILING);
    assert!(
        rms(&mixed) > 0.5 * rms(&boosted),
        "the limiter only shaves the peaks"
    );
}

#[test]
fn limiter_is_the_same_after_a_jump() {
    let dir = speech_dir();
    let project = speech_project();
    let levels = clip_levels(project.clips[0].id, "mic", level(1.0, GAIN_MAX, false));
    let mut mixer = mixer(&dir, &project, &levels);
    let through = render_all(&mut mixer, 1000);
    for start in [300_000_i64, 4_321, 700_000, 299_000] {
        let mut part = vec![0.0; 20_000 * CHANNELS];
        mixer.render(start, &mut part).unwrap();
        let at = start as usize * CHANNELS;
        let worst = worst_ratio(&part, &through[at..], 1.0);
        assert!(worst < 1e-6, "from {start}: off by {worst}");
    }
}

#[test]
fn mute_of_a_channel_and_of_a_clip() {
    let dir = speech_dir();
    let project = speech_project();
    let clip = project.clips[0].id;
    let found = tracks(&project_dir(&dir), &project.clips[0].media).unwrap();
    let frames = frame_at(20_016_666) as usize;
    let system = read(&found[0], 0, frames, 48_000);
    let levels = clip_levels(clip, "mic", level(1.0, 1.0, true));
    let mixed = render_all(&mut mixer(&dir, &project, &levels), 4096);
    assert!(
        worst_ratio(&mixed[..system.len()], &system, 1.0) < 1e-6,
        "mic muted: system only"
    );
    let mut levels = Levels::default();
    levels.clips.insert(
        clip,
        ClipLevels {
            muted: true,
            ..ClipLevels::default()
        },
    );
    let mixed = render_all(&mut mixer(&dir, &project, &levels), 4096);
    assert_eq!(peak(&mixed), 0.0, "clip muted: silence");
}

#[test]
fn muted_clip_next_to_a_loud_one_fades() {
    let (dir, mut project, _) = quiet();
    project.clips[0].trim = range(0.0, 1.0);
    add_clip(&mut project, "tone.mp4", 2.0).trim = range(1.0, 2.0);
    let mut levels = Levels::default();
    let second = project.clips[1].id;
    levels.clips.insert(
        second,
        ClipLevels {
            muted: true,
            ..ClipLevels::default()
        },
    );
    let mixed = render_all(&mut mixer(dir.path(), &project, &levels), 4096);
    let left = channel(&mixed, 0);
    let join = RATE as usize;
    assert!(peak(&left[join - 1000..join - 240]) > 0.04);
    assert!(peak(&left[join - 240..join]) > 0.0);
    assert_eq!(peak(&left[join + 240..]), 0.0, "after the fade-out");
    let slope = common::project::sharpest(&left, join - 480, join + 480);
    assert!(slope < 2.0 * 0.05 * (2.0 * std::f32::consts::PI * 440.0 / RATE as f32).powi(2) + 1e-4);
}

#[test]
fn denoise_is_a_filter_of_the_channel() {
    let dir = tempfile::tempdir().unwrap();
    clip(
        dir.path(),
        "hiss.mp4",
        "anoisesrc=color=pink:amplitude=0.03:r=48000:seed=3:d=3",
        &["-ac", "2"],
    );
    let mut project = Project::default();
    add_clip(&mut project, "hiss.mp4", 3.0);
    let clip = project.clips[0].id;
    let as_is = clip_levels(clip, "audio-1", level(1.0, 1.0, false));
    let denoised = clip_levels(clip, "audio-1", level(1.0, 1.0, false).with_denoise(true));
    let plain = render_all(&mut mixer(dir.path(), &project, &as_is), 4096);
    let clean = render_all(&mut mixer(dir.path(), &project, &denoised), 4096);
    let tail = |s: &[f32]| rms(&s[RATE as usize * CHANNELS..]);
    let change = 20.0 * (tail(&clean) / tail(&plain)).log10();
    assert!(change < -20.0, "hiss after the warm-up: {change:.1} dB");
}
