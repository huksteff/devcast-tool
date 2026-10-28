//! A recording survives its process being killed.
//!
//! `#[ignore]`: records monitor `DEVCAST_TEST_MONITOR` in a child process (this test binary
//! started again with `child_records_until_killed`), kills it with TerminateProcess in the
//! middle, recovers the folder and checks the file with ffprobe.
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --run-ignored=all --no-capture`;
//! look at the printed PNG, then delete the folder.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::probe::{AudioProbe, Probe, frame_png};
use common::{Running, TestPicture, fresh_dir, test_monitor};
use dc_capture::win::Recording;
use dc_capture::{Manifest, RecordingConfig, recover};

/// Set for the child: the folder to record into.
const CHILD_DIR: &str = "DEVCAST_TEST_RECORD_DIR";
const STARTED: &str = "child recording started";

/// The child half of `recording_survives_kill`; does nothing when run on its own.
#[test]
#[ignore = "child process of recording_survives_kill"]
fn child_records_until_killed() {
    let Ok(dir) = std::env::var(CHILD_DIR) else {
        return;
    };
    let monitor = test_monitor();
    let config = RecordingConfig::new(monitor.source());
    let _recording = Recording::start(&config, dir.as_ref()).unwrap();
    println!("{STARTED}");
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// The system sound track is recovered with the length of its complete segments.
fn check_sound(path: &std::path::Path, kept: f64, seconds: f64) {
    let audio = AudioProbe::read(path);
    println!("recovered {}: {audio:?}", path.display());
    audio.check_common();
    assert!((audio.duration - kept).abs() < 0.001, "{audio:?} vs {kept}");
    assert!(audio.duration > seconds - 3.5, "lost more than a segment");
}

#[test]
#[ignore = "records the screen: needs DEVCAST_TEST_MONITOR"]
fn recording_survives_kill() {
    let monitor = test_monitor();
    let _picture = TestPicture::show(&monitor, 30);
    let dir = fresh_dir("recover-kill");
    let mut child = Running(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "child_records_until_killed",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD_DIR, &dir)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let started = BufReader::new(stdout)
        .lines()
        .map(Result::unwrap)
        .any(|line| line.contains(STARTED));
    assert!(started, "the child did not start recording");
    let recorded = Instant::now();
    std::thread::sleep(Duration::from_secs(7));
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let seconds = recorded.elapsed().as_secs_f64();

    let manifest = Manifest::read(&dir).unwrap();
    let track = &manifest.tracks[0];
    let listed: Vec<&str> = track.segments.iter().map(|s| s.file.as_str()).collect();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    println!("killed after {seconds:.1} s: files {names:?}, complete segments {listed:?}");
    assert!(!manifest.is_finished());
    let kept = track.end() as f64 / 60.0;
    let sound = &manifest.tracks[1];
    assert_eq!(sound.name, "system");
    let sound_kept = sound.end() as f64 / 48_000.0;

    let files = recover(&dir).unwrap();
    let probe = Probe::read(&files[0]);
    println!(
        "recovered {}: {} frames, {:.2} s of {seconds:.1} s, {:.1} fps, key frames {:?}\n  {}",
        files[0].display(),
        probe.times.len(),
        probe.duration,
        probe.frame_rate(),
        probe.keys,
        frame_png(&files[0], 2.0).display(),
    );
    probe.check_common();
    assert!(
        (probe.duration - kept).abs() < 0.01,
        "{} vs {kept}",
        probe.duration
    );
    assert!(
        probe.duration > seconds - 3.5,
        "lost more than a segment and a GOP"
    );
    assert!(probe.frame_rate() > 50.0);
    check_sound(&files[1], sound_kept, seconds);
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(
        left,
        ["manifest.json", "system.mp4", "video.2s.png", "video.mp4"]
    );
}
