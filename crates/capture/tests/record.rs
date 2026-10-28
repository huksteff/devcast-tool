//! Recording the screen into a file on the real desktop of this computer.
//!
//! `#[ignore]`: draws a moving test picture on monitor `DEVCAST_TEST_MONITOR` and records it.
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --run-ignored=all --no-capture`;
//! the file and PNG paths are printed: look at the PNGs, then delete the files.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unsafe_code,
    reason = "test helpers: a failed step must fail the test; CPU time comes from Win32"
)]

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::probe::{Probe, frame_png, frame_rgb};
use common::{TestPicture, test_monitor};
use dc_capture::win::{Recording, VideoStart, VideoSummary};
use dc_capture::{Area, Fps, Manifest, RecordingConfig, Source};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

/// CPU time of this process (all threads), seconds.
fn cpu_seconds() -> f64 {
    let mut times = [FILETIME::default(); 4];
    let [created, exited, kernel, user] = &mut times;
    // SAFETY: the pseudo handle of the current process and four out slots.
    unsafe { GetProcessTimes(GetCurrentProcess(), created, exited, kernel, user) }.unwrap();
    let seconds = |t: &FILETIME| {
        ((u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)) as f64 / 1e7
    };
    seconds(kernel) + seconds(user)
}

struct Recorded {
    path: PathBuf,
    info: VideoStart,
    summary: VideoSummary,
    probe: Probe,
}

fn record(name: &str, source: &Source, fps: Fps, seconds: u64) -> Recorded {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let config = RecordingConfig {
        fps,
        system_audio: false,
        ..RecordingConfig::new(*source)
    };
    let recording = Recording::start(&config, &dir).unwrap();
    let (cpu, wall) = (cpu_seconds(), Instant::now());
    std::thread::sleep(Duration::from_secs(seconds));
    let info = recording.info();
    let finished = recording.stop().unwrap();
    let load = (cpu_seconds() - cpu) / wall.elapsed().as_secs_f64() * 100.0;
    let summary = finished.video;
    let path = finished.files[0].clone();
    assert_eq!(path, dir.join("video.mp4"));
    assert!(Manifest::read(&dir).unwrap().is_finished());
    let probe = Probe::read(&path);
    let megabytes = std::fs::metadata(&path).unwrap().len() as f64 / 1e6;
    println!(
        "{name}: {} time base {} (speed check {:.0} fps), {:?}, {} frames, {:.2} s, {:.1} fps, key frames {:?}, {megabytes:.1} MB, CPU {load:.0}% of one core\n  {}",
        info.codec,
        probe.time_base,
        info.speed,
        info.size,
        summary.frames,
        probe.duration,
        probe.frame_rate(),
        probe.keys,
        path.display(),
    );
    probe.check_common();
    let expected = summary.end as f64 / f64::from(fps.get());
    assert!(
        (probe.duration - expected).abs() < 0.01,
        "{} vs {expected}",
        probe.duration
    );
    assert!(
        (probe.duration - seconds as f64).abs() < 0.3,
        "{}",
        probe.duration
    );
    assert_eq!(probe.times.len() as u64, summary.frames);
    println!("  {}", frame_png(&path, 1.0).display());
    Recorded {
        path,
        info,
        summary,
        probe,
    }
}

#[test]
#[ignore = "records the screen: needs DEVCAST_TEST_MONITOR"]
fn records_monitor_area_and_window() {
    let monitor = test_monitor();
    let picture = TestPicture::show(&monitor, 40);
    let b = monitor.bounds;

    let screen = record("record-monitor-60", &monitor.source(), Fps::Fps60, 6);
    assert_eq!(
        screen.probe.size,
        (u64::from(b.width()), u64::from(b.height()))
    );
    assert!(screen.probe.frame_rate() > 50.0);

    // Odd size: NV12 needs even sides, the last column and row are dropped.
    let area = Area::new(b.x() + 100 + 320, b.y() + 100 + 180, 641, 361).unwrap();
    let cut = record("record-area-120", &Source::Area(area), Fps::Fps120, 4);
    assert_eq!(cut.info.size, (640, 360));
    assert_eq!(cut.probe.size, (640, 360));

    let window = record("record-window-30", &picture.window.source(), Fps::Fps30, 4);
    assert_eq!(window.probe.size, (1280, 720));
    let rate = window.probe.frame_rate();
    assert!((rate - 30.0).abs() < 2.0, "{rate}");
    assert!(window.summary.frames > 100);
    // The window holds only the moving test picture: a still recording means frames got lost.
    assert_ne!(
        frame_rgb(&window.path, 1.0),
        frame_rgb(&window.path, 3.0),
        "the test picture did not move: was ffplay covered or frozen?"
    );
    assert!(window.path.exists());
}
