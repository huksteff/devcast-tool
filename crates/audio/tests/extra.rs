//! Task 5.37: extra sound files on the output timeline: placed by output time whatever the cuts,
//! from their offset, at their level, normalized to −16 LUFS, with soft edges.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::path::{Path, PathBuf};

use common::project::{add_clip, mixer, range, render_all, sharpest};
use common::{channel, clip, make, onset, rms};
use dc_audio::{
    CHANNELS, ChannelLevel, ExtraClip, Levels, LoudnessCache, Meter, Mixer, RATE, TARGET,
};
use dc_core::{OutputTime, Project, TimeDelta};

const SILENCE: &str = "anullsrc=r=48000:cl=stereo:d=6";

/// A project of one 6 s clip of silence, the 4 s cut out of it from 1 s to 3 s when `cut`.
fn silent_project(dir: &Path, cut: bool) -> Project {
    clip(dir, "silence.mp4", SILENCE, &[]);
    let mut project = Project::default();
    let clip = add_clip(&mut project, "silence.mp4", 6.0);
    if cut {
        clip.cuts.push(range(1.0, 3.0));
    }
    project
}

/// A tone whose amplitude says the second of the file: 0.1 in 0–1 s, 0.2 in 1–2 s, …
fn steps(dir: &Path) -> PathBuf {
    make(
        dir,
        "шаги музыки.flac",
        "aevalsrc=0.1*(1+floor(t))*sin(2*PI*300*t):s=44100:d=4",
        &[],
    )
}

fn extra(path: &Path, from: f64, to: f64, offset: f64, level: ChannelLevel) -> ExtraClip {
    ExtraClip {
        path: path.to_path_buf(),
        start: OutputTime::from_secs_f64(from),
        end: OutputTime::from_secs_f64(to),
        offset: TimeDelta::from_secs_f64(offset),
        level,
    }
}

/// RMS of the left channel in output seconds `from..to`.
fn rms_at(mixed: &[f32], from: f64, to: f64) -> f64 {
    let frame = |s: f64| (s * f64::from(RATE)) as usize;
    rms(&channel(
        &mixed[frame(from) * CHANNELS..frame(to) * CHANNELS],
        0,
    ))
}

#[test]
fn a_file_plays_at_its_output_time_from_its_offset_and_cuts_do_not_move_it() {
    for cut in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let project = silent_project(dir.path(), cut);
        let music = steps(dir.path());
        let levels = Levels {
            extra: vec![extra(&music, 1.5, 3.5, 1.0, ChannelLevel::default())],
            ..Levels::default()
        };
        let mixed = render_all(&mut mixer(dir.path(), &project, &levels), 4096);
        let start = onset(&mixed, 0.01) as f64 / f64::from(RATE);
        assert!((start - 1.5).abs() < 0.005, "cut {cut}: starts at {start}");
        // Second 1–2 of the file (amplitude 0.2) plays at 1.5–2.5, second 2–3 (0.3) after.
        let sine = std::f64::consts::FRAC_1_SQRT_2;
        assert!(
            (rms_at(&mixed, 1.6, 2.4) - 0.2 * sine).abs() < 0.003,
            "cut {cut}"
        );
        assert!(
            (rms_at(&mixed, 2.6, 3.4) - 0.3 * sine).abs() < 0.003,
            "cut {cut}"
        );
        assert!(
            rms_at(&mixed, 3.51, 4.0) < 1e-4,
            "cut {cut}: plays past its end"
        );
    }
}

#[test]
fn level_mute_and_edges() {
    let dir = tempfile::tempdir().unwrap();
    let project = silent_project(dir.path(), false);
    let music = steps(dir.path());
    let quiet = ChannelLevel::new(0.5, 3.0).unwrap();
    let levels = Levels {
        extra: vec![
            extra(&music, 0.5, 1.5, 0.2508, quiet),
            extra(&music, 2.0, 3.0, 0.0, quiet.with_mute(true)),
        ],
        ..Levels::default()
    };
    let mixed = render_all(&mut mixer(dir.path(), &project, &levels), 1000);
    let expected = 0.1 * 1.5 * std::f64::consts::FRAC_1_SQRT_2;
    assert!((rms_at(&mixed, 0.6, 1.2) - expected).abs() < 0.002);
    assert!(rms_at(&mixed, 1.9, 3.1) < 1e-6, "a muted clip sounds");
    // Started inside the file (not at a zero of the tone), it fades in over 10 ms: no corner
    // sharper than the tone itself.
    let left = channel(&mixed, 0);
    let start = (0.5 * f64::from(RATE)) as usize;
    let tone = 0.15 * (2.0 * std::f32::consts::PI * 300.0 / RATE as f32).powi(2);
    assert!(sharpest(&left, start - 480, start + 480) < 2.0 * tone);
}

#[test]
fn a_normalized_file_measures_minus_16_lufs() {
    let dir = tempfile::tempdir().unwrap();
    clip(
        dir.path(),
        "silence.mp4",
        "anullsrc=r=48000:cl=stereo:d=8",
        &[],
    );
    let mut project = Project::default();
    add_clip(&mut project, "silence.mp4", 8.0);
    let noise = make(
        dir.path(),
        "noise.wav",
        "anoisesrc=a=0.02:c=pink:d=8:r=48000",
        &[],
    );
    let level = ChannelLevel::default().with_normalize(true);
    let levels = Levels {
        extra: vec![extra(&noise, 0.0, 8.0, 0.0, level)],
        ..Levels::default()
    };
    let folder = common::project::project_dir(dir.path());
    let mut mixer = Mixer::new(&folder, &project, &levels, &mut LoudnessCache::default()).unwrap();
    let frames = mixer.frames() as usize;
    let mixed = render_all(&mut mixer, 48_000);
    let mut meter = Meter::new().unwrap();
    meter.add(&mixed[..frames * CHANNELS]).unwrap();
    let loudness = meter.result().unwrap().integrated;
    assert!((loudness - TARGET).abs() < 0.2, "{loudness} LUFS");
}

/// A file placed from its first sample after a gap: its fade-in asks for the frames just before
/// the file, which are silence — not a seek before the start that an mp3 refuses.
#[test]
fn a_file_from_its_start_plays_after_a_gap() {
    for name in [
        "голос.mp3",
        "голос.flac",
        "голос.m4a",
        "голос.wav",
        "голос.ogg",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let project = silent_project(dir.path(), false);
        let voice = make(
            dir.path(),
            name,
            "sine=frequency=300:sample_rate=44100:d=2",
            &[],
        );
        let levels = Levels {
            extra: vec![extra(&voice, 1.0, 3.0, 0.0, ChannelLevel::default())],
            ..Levels::default()
        };
        let mixed = render_all(&mut mixer(dir.path(), &project, &levels), 1024);
        assert!(rms_at(&mixed, 0.0, 0.9) < 1e-4, "{name}: the gap is silent");
        assert!(rms_at(&mixed, 1.2, 2.8) > 0.05, "{name}: the file sounds");
    }
}

/// Playing again from the start of a file (the playhead back at a clip's first moment): the
/// reader goes back to before its first frame — to the start of the file, not to a time before
/// it, which a seek refuses.
#[test]
fn a_file_plays_again_from_its_start() {
    for name in [
        "голос.mp3",
        "голос.flac",
        "голос.m4a",
        "голос.wav",
        "голос.ogg",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let project = silent_project(dir.path(), false);
        let voice = make(
            dir.path(),
            name,
            "sine=frequency=300:sample_rate=44100:d=2",
            &[],
        );
        let levels = Levels {
            extra: vec![extra(&voice, 1.0, 3.0, 0.0, ChannelLevel::default())],
            ..Levels::default()
        };
        let mut mixer = mixer(dir.path(), &project, &levels);
        let second = RATE as usize * CHANNELS;
        let mut first = vec![0.0; second];
        mixer.render(i64::from(RATE), &mut first).unwrap();
        let mut again = vec![0.0; second];
        mixer.render(i64::from(RATE) * 2, &mut again).unwrap();
        mixer.render(i64::from(RATE), &mut again).unwrap();
        assert!(rms(&channel(&first, 0)) > 0.05, "{name}: the file sounds");
        assert!(first == again, "{name}: the same sound the second time");
    }
}
