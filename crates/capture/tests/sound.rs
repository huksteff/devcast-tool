//! Recording sound on the real devices of this computer.
//!
//! `#[ignore]`: records the test picture on monitor `DEVCAST_TEST_MONITOR` with system sound
//! while ffplay plays a quiet tone (−40 dBFS). Whatever else plays is recorded too: the tone
//! is found by its frequency.
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --run-ignored=all --no-capture`;
//! the folder is printed: delete it after looking.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::probe::{AudioProbe, Probe, audio_left};
use common::{Running, TestPicture, test_monitor, tone};
use dc_capture::audio::{RATE, list_microphones};
use dc_capture::win::Recording;
use dc_capture::{Fps, Manifest, RecordingConfig};

/// The test tone: an unusual frequency, so whatever else plays on this computer barely has it.
const TONE: u32 = 1234;

/// Amplitude of the `frequency` component of `samples` (one DFT bin), dBFS.
fn tone_db(samples: &[f32], frequency: u32) -> f64 {
    let step = std::f64::consts::TAU * f64::from(frequency) / f64::from(RATE);
    let (re, im) = samples
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(re, im), (n, s)| {
            let phase = step * n as f64;
            (
                re + f64::from(*s) * phase.cos(),
                im - f64::from(*s) * phase.sin(),
            )
        });
    let amplitude = 2.0 * (re * re + im * im).sqrt() / samples.len() as f64;
    20.0 * amplitude.max(1e-9).log10()
}

#[test]
#[ignore = "lists the sound devices of this computer"]
fn lists_microphones() {
    let microphones = list_microphones().unwrap();
    for m in &microphones {
        println!("microphone {:?} default={}", m.name, m.default);
    }
    assert!(microphones.iter().filter(|m| m.default).count() <= 1);
    assert!(microphones.iter().all(|m| !m.name.trim().is_empty()));
}

#[test]
#[ignore = "records the screen and system sound: needs DEVCAST_TEST_MONITOR"]
fn records_system_sound_on_the_video_clock() {
    let monitor = test_monitor();
    let picture = TestPicture::show(&monitor, 20);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sound-system");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let config = RecordingConfig {
        fps: Fps::Fps30,
        ..RecordingConfig::new(picture.window.source())
    };
    let recording = Recording::start(&config, &dir).unwrap();
    // Silence first: loopback sends nothing, the track must still start at the video's start.
    std::thread::sleep(Duration::from_secs(2));
    let player = Running(tone(TONE, 2.0, -40.0));
    std::thread::sleep(Duration::from_secs(4));
    drop(player);
    let summary = recording.stop().unwrap();
    println!("{dir:?}\n{summary:#?}");

    let manifest = Manifest::read(&dir).unwrap();
    let [video, system] = &manifest.tracks[..] else {
        panic!("{manifest:?}");
    };
    assert_eq!(video.start, system.start, "tracks start together");
    let probe = Probe::read(&summary.files[0]);
    let audio = AudioProbe::read(&summary.files[1]);
    println!("video {:.3} s, sound {audio:?}", probe.duration);
    audio.check_common();
    let sound = &summary.audio[0];
    assert!((audio.duration - sound.frames as f64 / f64::from(RATE)).abs() < 1e-6);
    // Both end at the stop; the video's end is rounded to its frame slots and its last frame
    // may be stamped ahead of the clock (decision 007, up to ~30 ms).
    assert!(
        (-0.05 - 1.0 / 30.0..=1.0 / 30.0).contains(&(audio.duration - probe.duration)),
        "{} vs {}",
        audio.duration,
        probe.duration
    );
    assert!(audio.duration > 5.9, "{audio:?}");

    let left = audio_left(&summary.files[1]);
    let second = RATE as usize;
    let before = tone_db(&left[..second], TONE);
    let during = tone_db(
        &left[2 * second + second / 2..3 * second + second / 2],
        TONE,
    );
    println!("{TONE} Hz: first second {before:.1} dBFS, during the tone {during:.1} dBFS");
    assert!(before < -60.0, "{before}");
    assert!((-46.0..-36.0).contains(&during), "{during}");
    assert!(sound.stats.worst_offset < 0.005, "{sound:?}");
    assert!(sound.problems.is_empty(), "{sound:?}");
}
