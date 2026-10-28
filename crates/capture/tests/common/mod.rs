//! Shared helpers of the live tests: the allowed monitor and a moving test picture on it.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "helpers shared by test files, each using its own part; a failed step must fail the test"
)]

pub mod probe;
pub mod synthetic;

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use dc_capture::win::list_sources;
use dc_capture::{MonitorInfo, WindowInfo};

pub const TITLE: &str = "devcast-live-test";

/// The monitor the owner allowed to record.
pub fn test_monitor() -> MonitorInfo {
    let index: u32 = std::env::var("DEVCAST_TEST_MONITOR")
        .expect(
            "set DEVCAST_TEST_MONITOR=<n>: the monitor (from 1) this test may draw on and record",
        )
        .parse()
        .expect("DEVCAST_TEST_MONITOR must be a monitor number");
    let sources = list_sources().unwrap();
    sources
        .monitors
        .into_iter()
        .find(|m| m.index == index)
        .expect("no such monitor")
}

/// `name` in the test temp folder, with what an earlier run left there deleted. Only tests
/// have the folder; `examples/record_fixtures` shares this module too.
#[cfg(test)]
pub fn fresh_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    dir
}

fn ffplay() -> PathBuf {
    PathBuf::from(env!("FFMPEG_DIR")).join("bin/ffplay.exe")
}

/// ffplay playing a `frequency` Hz tone for `seconds` at `dbfs` peak, without a window.
pub fn tone(frequency: u32, seconds: f64, dbfs: f64) -> Child {
    // `sine` peaks at 1/8 of full scale, −18.06 dBFS.
    let gain = dbfs + 18.06;
    Command::new(ffplay())
        .args([
            "-loglevel",
            "quiet",
            "-nodisp",
            "-autoexit",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg(format!(
            "sine=frequency={frequency}:duration={seconds}:sample_rate=48000,volume={gain}dB"
        ))
        .spawn()
        .unwrap()
}

/// ffplay showing `testsrc2` 1280×720 at 60 fps without a frame, 100 px from the monitor corner.
pub struct TestPicture {
    _process: Running,
    pub window: WindowInfo,
}

/// Kills the process when the test ends, also on panic.
pub struct Running(pub Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl TestPicture {
    pub fn show(monitor: &MonitorInfo, seconds: u32) -> Self {
        let ffplay = ffplay();
        let process = Running(
            Command::new(&ffplay)
                .args([
                    "-loglevel",
                    "quiet",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=1280x720:rate=60",
                ])
                .args([
                    "-noborder",
                    "-alwaysontop",
                    "-autoexit",
                    "-window_title",
                    TITLE,
                ])
                .args(["-left", &(monitor.bounds.x() + 100).to_string()])
                .args(["-top", &(monitor.bounds.y() + 100).to_string()])
                .args(["-t", &seconds.to_string()])
                .spawn()
                .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask deps`", ffplay.display())),
        );
        // ffplay gives no signal when its window is shown, so the list is polled.
        let started = Instant::now();
        loop {
            let windows = list_sources().unwrap().windows;
            if let Some(window) = windows.into_iter().find(|w| w.title == TITLE) {
                return Self {
                    _process: process,
                    window,
                };
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "ffplay window did not appear"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
