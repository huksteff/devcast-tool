//! `devcast record` for real: records monitor `DEVCAST_TEST_MONITOR` through the binary and
//! controls it from other calls.
//!
//! `#[ignore]`: records the screen. Run when nobody works at the computer:
//! `DEVCAST_TEST_MONITOR=2 cargo nextest run -p dc-cli --test record_live --run-ignored=all --no-capture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use dc_project::ProjectDir;

fn monitor() -> String {
    let index = std::env::var("DEVCAST_TEST_MONITOR")
        .expect("set DEVCAST_TEST_MONITOR=<n>: the monitor (from 1) this test may record");
    format!("screen:{index}")
}

fn devcast(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_devcast"))
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Sends `control` to the recording started in the background, once it listens.
fn control(cwd: &Path, control: &str) -> String {
    let started = Instant::now();
    loop {
        let out = devcast(cwd, &["record", control]);
        if out.status.success() {
            return text(&out.stdout);
        }
        let error = text(&out.stderr);
        assert!(
            error.contains("запись не идёт"),
            "record {control}: {error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the background devcast record did not start listening"
        );
        // The background process is starting: it takes the channel within milliseconds.
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "records the screen: needs DEVCAST_TEST_MONITOR"]
fn records_pauses_stops_and_cancels_through_the_cli() {
    let source = monitor();
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path();

    let out = devcast(
        cwd,
        &[
            "record",
            "--source",
            &source,
            "--fps",
            "30",
            "--duration",
            "3",
            "-o",
            "a.dcproj",
        ],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    println!("{}", text(&out.stdout));
    let project = ProjectDir::open(cwd.join("a.dcproj"))
        .unwrap()
        .load()
        .unwrap();
    let seconds = project.clips[0].media.duration.as_secs_f64();
    assert!((seconds - 3.0).abs() < 0.3, "{seconds}");

    let background = Command::new(env!("CARGO_BIN_EXE_devcast"))
        .args(["record", "--source", &source, "-o", "b.dcproj", "--json"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    println!("{}", control(cwd, "pause"));
    std::thread::sleep(Duration::from_secs(2));
    println!("{}", control(cwd, "resume"));
    std::thread::sleep(Duration::from_secs(2));
    let stopped = control(cwd, "stop");
    println!("{stopped}");
    assert!(stopped.contains("пауз: 1"), "{stopped}");
    let finished = background.wait_with_output().unwrap();
    assert!(finished.status.success());
    let json: serde_json::Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(json["pauses"], 1);
    let seconds = json["duration"].as_f64().unwrap();
    assert!((seconds - 2.0).abs() < 0.5, "pause is cut out: {seconds}");

    let mut background = Command::new(env!("CARGO_BIN_EXE_devcast"))
        .args(["record", "--source", &source, "-o", "c.dcproj"])
        .current_dir(cwd)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    control(cwd, "pause");
    println!("{}", control(cwd, "cancel"));
    assert!(background.wait().unwrap().success());
    assert!(!cwd.join("c.dcproj").exists());
}
