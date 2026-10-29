//! Recording sound on the real devices of this computer.
//!
//! `#[ignore]`: records the test picture on monitor `DEVCAST_TEST_MONITOR` with system sound
//! while ffplay plays a quiet tone (−40 dBFS). Whatever else plays is recorded too: the tone
//! is found by its frequency.
//! The microphone test records the room: it prints only measurements and deletes its files.
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --run-ignored=all --no-capture`;
//! the folder of the system sound test is printed: delete it after looking.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::probe::{AudioProbe, Probe, audio_left};
use common::{Running, TestPicture, fresh_dir, test_monitor, tone};
use dc_capture::audio::{FLOOR_DB, LEVELS_PER_SECOND, LevelMeter, RATE, list_microphones};
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

fn default_microphone() -> String {
    let microphones = list_microphones().unwrap();
    microphones
        .into_iter()
        .find(|m| m.default)
        .expect("no default microphone")
        .name
}

fn rms_db(samples: &[f32]) -> f64 {
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    10.0 * (sum / samples.len() as f64).max(1e-20).log10()
}

#[test]
#[ignore = "listens to the default microphone"]
fn meters_microphone_level() {
    let (sender, levels) = mpsc::channel();
    let meter = LevelMeter::start(&default_microphone(), sender).unwrap();
    let started = Instant::now();
    std::thread::sleep(Duration::from_secs(2));
    drop(meter);
    let seconds = started.elapsed().as_secs_f64();
    let levels: Vec<_> = levels.try_iter().map(Result::unwrap).collect();
    let rate = levels.len() as f64 / seconds;
    let loudest = levels.iter().map(|l| l.peak_db).fold(FLOOR_DB, f32::max);
    let mut rms: Vec<f32> = levels.iter().map(|l| l.rms_db).collect();
    rms.sort_by(f32::total_cmp);
    println!(
        "{} levels in {seconds:.2} s ({rate:.1}/s), RMS median {:.1} dBFS, peak {loudest:.1} dBFS",
        levels.len(),
        rms[rms.len() / 2]
    );
    assert!((rate - f64::from(LEVELS_PER_SECOND)).abs() < 3.0, "{rate}");
    assert!(
        levels
            .iter()
            .all(|l| (FLOOR_DB..=0.0).contains(&l.rms_db) && l.rms_db <= l.peak_db)
    );
}

#[test]
#[ignore = "records the screen and the microphone: needs DEVCAST_TEST_MONITOR"]
fn records_microphone_track() {
    let monitor = test_monitor();
    let picture = TestPicture::show(&monitor, 20);
    let dir = fresh_dir("sound-mic");
    let config = RecordingConfig {
        fps: Fps::Fps30,
        system_audio: false,
        microphone: Some(default_microphone()),
        ..RecordingConfig::new(picture.window.source())
    };
    let recording = Recording::start(&config, &dir).unwrap();
    std::thread::sleep(Duration::from_secs(5));
    let summary = recording.stop().unwrap();
    let manifest = Manifest::read(&dir).unwrap();
    let names: Vec<&str> = manifest.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["video", "mic"]);
    let audio = AudioProbe::read(&summary.files[1]);
    let room = rms_db(&audio_left(&summary.files[1]));
    // The room is not kept: only the numbers.
    std::fs::remove_dir_all(&dir).unwrap();
    let mic = &summary.audio[0];
    println!(
        "{mic:#?}
{audio:?}, room RMS {room:.1} dBFS"
    );
    audio.check_common();
    assert!((audio.duration - mic.frames as f64 / f64::from(RATE)).abs() < 1e-6);
    assert!(audio.duration > 4.9, "{audio:?}");
    assert!(mic.problems.is_empty(), "{mic:?}");
    assert!(mic.stats.worst_offset < 0.005, "{mic:?}");
    // A working microphone always hears some noise; digital silence means it records nothing.
    assert!(room > -110.0, "{room}");
}

#[test]
#[ignore = "records the screen and system sound: needs DEVCAST_TEST_MONITOR"]
fn records_system_sound_on_the_video_clock() {
    let monitor = test_monitor();
    let picture = TestPicture::show(&monitor, 20);
    let dir = fresh_dir("sound-system");
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
    println!("{dir:?}\n{:#?}", summary.audio);

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
    // Both end at the stop; the video's end is rounded to its frame slots.
    assert!(
        (audio.duration - probe.duration).abs() <= 1.0 / 30.0,
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
    // Played at −40 dBFS, but Windows scales it before the loopback (the mixer volume of ffplay,
    // lowering of other sounds during a call), so only "clearly there" is checked.
    assert!(during > -70.0 && during < -36.0, "{during}");
    assert!(sound.stats.worst_offset < 0.005, "{sound:?}");
    assert!(sound.problems.is_empty(), "{sound:?}");
}
