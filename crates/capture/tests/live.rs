//! Checks on the real desktop of this computer.
//!
//! Tests that capture the screen are `#[ignore]`: they draw a moving test picture (ffplay from
//! `cargo xtask deps`) on monitor `DEVCAST_TEST_MONITOR` and record it. Run them with
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --run-ignored=all --no-capture`;
//! the PNG paths are printed, look at them.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use dc_capture::win::{list_sources, preview_monitor, preview_window};
use dc_capture::{MonitorInfo, Preview, WindowInfo};
use windows_capture::encoder::{ImageEncoder, ImageEncoderPixelFormat, ImageFormat};

const TITLE: &str = "devcast-live-test";

/// The monitor the owner allowed to record.
fn test_monitor() -> MonitorInfo {
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

/// ffplay showing `testsrc2` 1280×720 at 60 fps without a frame, 100 px from the monitor corner.
struct TestPicture {
    _process: Running,
    window: WindowInfo,
}

/// Kills the process when the test ends, also on panic.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl TestPicture {
    fn show(monitor: &MonitorInfo, seconds: u32) -> Self {
        let ffplay = PathBuf::from(env!("FFMPEG_DIR")).join("bin/ffplay.exe");
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

fn save_png(name: &str, width: u32, height: u32, pixels: &[u8], format: ImageEncoderPixelFormat) {
    let png = ImageEncoder::new(ImageFormat::Png, format)
        .unwrap()
        .encode(pixels, width, height);
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, png.unwrap()).unwrap();
    println!("saved {}", path.display());
}

fn distinct_colors(preview: &Preview) -> usize {
    let mut colors: Vec<&[u8; 4]> = preview.rgba.as_chunks::<4>().0.iter().collect();
    colors.sort_unstable();
    colors.dedup();
    colors.len()
}

#[test]
fn lists_monitors_and_windows() {
    let sources = list_sources().unwrap();
    assert!(!sources.monitors.is_empty());
    assert_eq!(sources.monitors.iter().filter(|m| m.primary).count(), 1);
    assert!(sources.primary().is_some());
    let indices: Vec<u32> = sources.monitors.iter().map(|m| m.index).collect();
    assert_eq!(indices, (1..=indices.len() as u32).collect::<Vec<_>>());
    let mut ids: Vec<u64> = sources.windows.iter().map(|w| w.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), sources.windows.len());
    assert!(sources.windows.iter().all(|w| !w.title.trim().is_empty()));
    for m in &sources.monitors {
        println!(
            "monitor {} {:?} {:?} primary={}",
            m.index, m.name, m.bounds, m.primary
        );
    }
    for w in &sources.windows {
        println!(
            "window {} {:?} «{}» minimized={}",
            w.id, w.app, w.title, w.minimized
        );
    }
}

#[test]
#[ignore = "records the screen: needs DEVCAST_TEST_MONITOR"]
fn previews_monitor_and_window() {
    let monitor = test_monitor();
    let picture = TestPicture::show(&monitor, 10);
    assert_eq!(picture.window.app.as_deref(), Some("ffplay"));

    let screen = preview_monitor(monitor.index, 320).unwrap();
    assert_eq!(screen.width.max(screen.height), 320);
    let aspect = |w: u32, h: u32| f64::from(w) / f64::from(h);
    let monitor_aspect = aspect(monitor.bounds.width(), monitor.bounds.height());
    assert!((aspect(screen.width, screen.height) - monitor_aspect).abs() < 0.02);
    assert!(
        distinct_colors(&screen) > 100,
        "monitor preview looks empty"
    );
    save_png(
        "preview-monitor.png",
        screen.width,
        screen.height,
        &screen.rgba,
        ImageEncoderPixelFormat::Rgba8,
    );

    let window = preview_window(picture.window.id, 320).unwrap();
    assert_eq!((window.width, window.height), (320, 180));
    assert!(distinct_colors(&window) > 100, "window preview looks empty");
    save_png(
        "preview-window.png",
        window.width,
        window.height,
        &window.rgba,
        ImageEncoderPixelFormat::Rgba8,
    );
}
