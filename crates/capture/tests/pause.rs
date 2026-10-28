//! Pause, resume and cancel of a recording on the real desktop of this computer.
//!
//! `#[ignore]`: draws a moving test picture on monitor `DEVCAST_TEST_MONITOR` and records it.
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --test pause --run-ignored=all --no-capture`.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::probe::Probe;
use common::{TestPicture, test_monitor};
use dc_capture::win::Recording;
use dc_capture::{Fps, Manifest, RecordingConfig};

fn folder(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    dir
}

#[test]
#[ignore = "records the screen: needs DEVCAST_TEST_MONITOR"]
fn pause_is_cut_out_and_cancel_deletes_everything() {
    let monitor = test_monitor();
    let picture = TestPicture::show(&monitor, 30);
    let config = RecordingConfig {
        fps: Fps::Fps30,
        ..RecordingConfig::new(picture.window.source())
    };

    let dir = folder("pause");
    let recording = Recording::start(&config, &dir).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let paused = recording.pause().unwrap();
    assert!(recording.pause().is_err());
    std::thread::sleep(Duration::from_secs(3));
    let resumed = recording.resume().unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let summary = recording.stop().unwrap();
    let pause = summary.manifest.pauses.first().copied().unwrap();
    assert_eq!((pause.at, pause.resumed), (paused, Some(resumed)));
    assert_eq!(Manifest::read(&dir).unwrap(), summary.manifest);
    let probe = Probe::read(&summary.files[0]);
    println!(
        "{:?}: {:.2} s, {} frames, sound {:?}",
        summary.files,
        probe.duration,
        summary.video.frames,
        summary.audio.iter().map(|a| a.frames).collect::<Vec<_>>()
    );
    assert!((probe.duration - 4.0).abs() < 0.2, "{}", probe.duration);
    for audio in &summary.audio {
        let seconds = audio.frames as f64 / 48_000.0;
        assert!((seconds - probe.duration).abs() < 0.05, "{seconds}");
    }
    // The moving picture goes on right after the pause: no frame stands for the pause.
    let gap = probe
        .times
        .windows(2)
        .map(|t| t[1] - t[0])
        .fold(0.0, f64::max);
    assert!(gap < 0.2, "longest frame {gap} s");
    std::fs::remove_dir_all(&dir).unwrap();

    let dir = folder("cancel");
    let recording = Recording::start(&config, &dir).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    recording.pause().unwrap();
    recording.cancel().unwrap();
    assert!(!dir.exists(), "{} is left", dir.display());
}
