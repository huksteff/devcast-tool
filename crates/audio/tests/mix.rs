//! Task 4.6: the mix follows the time map; joins have no clicks; speed keeps the pitch.

mod common;

use common::project::{add_clip, range, render_all, sharpest, speech_dir, speech_project, speed};
use common::{channel, frequency, make, read, track};
use dc_audio::{CHANNELS, Levels, Mixer, RATE, frame_at, tracks};
use dc_core::Project;

const TONE: &str = "aevalsrc=0.5*sin(2*PI*440*t)|0.5*sin(2*PI*440*t):s=48000:d=4";

/// Second difference of a 0.5 sine at 440 Hz: the sharpest corner of clean sound.
fn smooth() -> f32 {
    0.5 * (2.0 * std::f32::consts::PI * 440.0 / RATE as f32).powi(2)
}

fn frame(seconds: f64) -> usize {
    frame_at((seconds * 1e6).round() as i64) as usize
}

#[test]
fn cut_has_no_click() {
    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), "tone.flac", TONE, &[]);
    let mut project = Project::default();
    add_clip(&mut project, "tone.flac", 4.0).cuts = vec![range(1.0037, 1.6211)];
    let mut mixer = Mixer::new(dir.path(), &project, &Levels::default()).unwrap();
    assert_eq!(mixer.frames(), frame_at(4_000_000 - 617_400));
    let left = channel(&render_all(&mut mixer, 1024), 0);
    let join = frame(1.0037);
    let at_join = sharpest(&left, join - 480, join + 480);
    let source = channel(
        &read(&track(&dir.path().join("tone.flac")), 0, 4 * 48_000, 4096),
        0,
    );
    let hard_cut = (source[frame(1.6211)] - source[frame(1.0037) - 1]).abs();
    println!(
        "join: sharpest {at_join:.5} (clean sine {:.5}); a hard cut would jump by {hard_cut:.3}",
        smooth()
    );
    assert!(at_join < 2.0 * smooth(), "click at the join: {at_join}");
    let level = common::rms(&left[join - 4800..join + 4800]);
    assert!(
        (level - 0.5 / 2f64.sqrt()).abs() < 0.02,
        "level dips: {level}"
    );
}

#[test]
fn join_inside_continuous_sound_is_transparent() {
    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), "tone.flac", TONE, &[]);
    let mut project = Project::default();
    add_clip(&mut project, "tone.flac", 4.0).trim = range(0.0, 1.5);
    add_clip(&mut project, "tone.flac", 4.0).trim = range(1.5, 4.0);
    let mixed = render_all(
        &mut Mixer::new(dir.path(), &project, &Levels::default()).unwrap(),
        1000,
    );
    let source = read(&track(&dir.path().join("tone.flac")), 0, 4 * 48_000, 4096);
    let worst = mixed
        .iter()
        .zip(&source)
        .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(
        worst < 1e-6,
        "split clip differs from the source by {worst}"
    );
}

#[test]
fn speed_keeps_pitch_length_and_joins() {
    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), "tone.flac", TONE, &[]);
    for factor in [0.25, 2.0, 16.0] {
        let mut project = Project::default();
        add_clip(&mut project, "tone.flac", 4.0).speeds = vec![speed(1.0, 2.0, factor)];
        let mut mixer = Mixer::new(dir.path(), &project, &Levels::default()).unwrap();
        let length = 3.0 + 1.0 / factor;
        assert_eq!(mixer.frames(), frame_at((length * 1e6).round() as i64));
        let left = channel(&render_all(&mut mixer, 2048), 0);
        let fast = &left[frame(1.0) + 480..frame(1.0 + 1.0 / factor) - 480];
        let hz = frequency(fast);
        let joins = [frame(1.0), frame(1.0 + 1.0 / factor)];
        let sharp = joins.map(|j| sharpest(&left, j - 480, j + 480));
        println!("speed {factor}: {hz:.1} Hz, sharpest at joins {sharp:?}");
        assert!((hz - 440.0).abs() < 5.0, "speed {factor}: pitch {hz}");
        assert!(sharp.iter().all(|s| *s < 2.0 * smooth()), "speed {factor}");
        assert!((common::rms(fast) - 0.5 / 2f64.sqrt()).abs() < 0.03);
    }
}

#[test]
fn jumps_give_the_same_samples_as_playing_through() {
    let dir = tempfile::tempdir().unwrap();
    let source = "anoisesrc=sample_rate=44100:duration=4:amplitude=0.5:seed=3";
    make(dir.path(), "noise.wav", source, &["-ac", "2"]);
    let mut project = Project::default();
    add_clip(&mut project, "noise.wav", 4.0).cuts = vec![range(0.5, 0.7), range(2.0, 2.9)];
    add_clip(&mut project, "noise.wav", 4.0).trim = range(1.0, 3.0);
    let mut mixer = Mixer::new(dir.path(), &project, &Levels::default()).unwrap();
    let through = render_all(&mut mixer, 1024);
    for start in [100_000_i64, 3_000, 140_000, 23_999, 90_000] {
        let mut part = vec![0.0; 9_000 * CHANNELS];
        mixer.render(start, &mut part).unwrap();
        let at = start as usize * CHANNELS;
        assert_eq!(part, through[at..at + part.len()], "from {start}");
    }
}

#[test]
fn recording_mixes_system_and_mic() {
    let dir = speech_dir();
    let project = speech_project();
    let mut mixer = Mixer::new(&dir, &project, &Levels::default()).unwrap();
    assert_eq!(mixer.frames(), frame_at(20_016_666));
    let mixed = render_all(&mut mixer, 4800);
    let found = tracks(&dir, &project.clips[0].media).unwrap();
    let frames = mixer.frames() as usize;
    let system = read(&found[0], 0, frames, 48_000);
    let mic = read(&found[1], 0, frames, 48_000);
    let worst = (0..frames * CHANNELS)
        .map(|i| (mixed[i] - system[i] - mic[i]).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 1e-6, "mix is not system + mic: {worst}");
    assert!(common::peak(&mixed) > 0.05);
}
