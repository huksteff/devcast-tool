//! Video and sound do not drift apart: a flash every second on the screen and a beep every
//! second in the speakers, both at the start of each second of the host clock (QPC), must keep
//! their distance in the recording within a frame from the first minutes to the last.
//!
//! The reference is on the recording's clock: the test's own window turns white for 50 ms at
//! every QPC second (`common/flash.rs`), and the test plays the beeps through cpal, placing
//! each at its QPC second by the playback timestamps. The distance «beep − flash» is what the
//! screen and the speakers add (a vsync or two, the output latency); it must stay put. The
//! WGC lead of every frame is printed with the shifts, to tell them apart.
//!
//! `#[ignore]`: draws on and records monitor `DEVCAST_TEST_MONITOR` with system sound (beeps
//! 17 kHz, 50 ms, −30 dBFS). Length `DEVCAST_SYNC_SECONDS` (90 by default; the nightly check
//! is 1800). The beep is found by its frequency, so other sound on the computer does not
//! matter.
//! `DEVCAST_TEST_MONITOR=<n> DEVCAST_SYNC_SECONDS=1800 cargo nextest run -p dc-capture --run-ignored=all --no-capture stays_in_sync`
//! The folder is deleted at the end; only the numbers are printed.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unsafe_code,
    reason = "test helpers: a failed step must fail the test; the flash window is Win32"
)]

mod common;
#[path = "common/flash.rs"]
mod flash;

use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use common::probe::{Probe, tool};
use common::{Running, fresh_dir, test_monitor};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dc_capture::audio::RATE;
use dc_capture::win::{Recording, VideoSummary};
use dc_capture::{Area, Fps, Manifest, RecordingConfig, Source};
use flash::Flasher;

/// High enough that music and speech barely have it (lossy audio stops at ~16 kHz).
const BEEP: f64 = 17_000.0;
/// Beep length, s.
const BEEP_LENGTH: f64 = 0.05;
/// Envelope smoothing of the beep detector: 5 ms.
const WINDOW: usize = 240;
/// Envelope above this is a beep or other sound at its frequency (the beeps reach ~0.03).
const THRESHOLD: f64 = 0.005;
const FPS: f64 = 60.0;

/// Plays a 50 ms beep at the start of every second of the host clock (QPC) on the default
/// output, at −30 dBFS.
fn play_beeps() -> cpal::Stream {
    let device = cpal::default_host().default_output_device().unwrap();
    let config = device.default_output_config().unwrap().config();
    let (rate, channels) = (u128::from(config.sample_rate), usize::from(config.channels));
    // The phase runs on with the samples: playback times jitter by tens of µs between
    // callbacks, which is half a period at 17 kHz.
    let mut played = 0_u64;
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let first = info.timestamp().playback.as_nanos();
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    let at = first + i as u128 * 1_000_000_000 / rate;
                    let into = (at % 1_000_000_000) as f64 / 1e9;
                    let phase = std::f64::consts::TAU * BEEP * played as f64 / rate as f64;
                    let sample = if into < BEEP_LENGTH {
                        0.0316 * phase.sin()
                    } else {
                        0.0
                    };
                    frame.fill(sample as f32);
                    played += 1;
                }
            },
            |e| panic!("beeps: {e}"),
            None,
        )
        .unwrap();
    stream.play().unwrap();
    stream
}

/// Times (s from the track start) of the frames where the flash appears.
fn flashes(path: &Path) -> Vec<f64> {
    let times = Probe::read(path).times;
    let output = Command::new(tool("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-vf", "scale=4:4,format=gray", "-fps_mode", "passthrough"])
        .args(["-f", "rawvideo", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let bright: Vec<bool> = output
        .stdout
        .chunks(16)
        .map(|f| f.iter().map(|v| u32::from(*v)).sum::<u32>() / 16 > 128)
        .collect();
    assert_eq!(bright.len(), times.len());
    (1..bright.len())
        .filter(|&i| bright[i] && !bright[i - 1])
        .map(|i| times[i])
        .collect()
}

/// Times (s from the track start) where the beeps begin. The decoded sound is streamed: half
/// an hour of it is too much to hold.
fn beeps(path: &Path) -> Vec<f64> {
    let mut ffmpeg = Running(
        Command::new(tool("ffmpeg"))
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-af", "pan=mono|c0=c0", "-f", "f32le", "-"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut samples = BufReader::new(ffmpeg.0.stdout.take().unwrap());
    let step = std::f64::consts::TAU * BEEP / f64::from(RATE);
    // Amplitude of the beep frequency over the last WINDOW samples, as a running sum.
    let mut ring = vec![(0.0, 0.0); WINDOW];
    let (mut re, mut im) = (0.0, 0.0);
    let mut region: Vec<f64> = Vec::new();
    let mut onsets = Vec::new();
    let mut bytes = [0u8; 4];
    let mut n = 0;
    while samples.read_exact(&mut bytes).is_ok() {
        let sample = f64::from(f32::from_le_bytes(bytes));
        let phase = step * n as f64;
        let (r, i) = (sample * phase.cos(), -sample * phase.sin());
        let (old_r, old_i) = std::mem::replace(&mut ring[n % WINDOW], (r, i));
        (re, im) = (re + r - old_r, im + i - old_i);
        let envelope = 2.0 * (re * re + im * im).sqrt() / WINDOW as f64;
        n += 1;
        if envelope >= THRESHOLD {
            region.push(envelope);
            continue;
        }
        if region.is_empty() {
            continue;
        }
        let peak = region.iter().copied().fold(0.0, f64::max);
        let half = region.iter().position(|e| *e >= 0.5 * peak).unwrap();
        let length = region.len() as f64 / f64::from(RATE);
        // A beep; anything else at this frequency is other sound (or an echo).
        if (BEEP_LENGTH - 0.01..BEEP_LENGTH + 0.015).contains(&length) {
            // The envelope at sample k covers k − WINDOW..k; a beep reaches half of it
            // WINDOW / 2 in.
            let first = n - 1 - region.len();
            onsets.push((first + half + 1 - WINDOW / 2) as f64 / f64::from(RATE));
        }
        region.clear();
    }
    onsets
}

/// Mean: a flash is shown at one of two vsyncs, the mean follows the mix without jumping a
/// whole frame as a median would.
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// One flash with the beep after it.
struct Pair {
    /// Flash time in the video, s.
    at: f64,
    /// Beep − flash of the same QPC second, ms, with the frame placed at its arrival (as
    /// recorded).
    arrival: f64,
    /// The same with the frame at its WGC time (arrival + its lead).
    shown: f64,
    /// WGC lead of the flash frame, ms.
    lead: f64,
}

fn pairs(flashes: &[f64], beeps: &[f64], video: &VideoSummary) -> Vec<Pair> {
    flashes
        .iter()
        .filter_map(|f| {
            let b = beeps.iter().find(|b| *b >= f)?;
            let slot = (f * FPS).round() as i64;
            let lead = video.leads.iter().find(|l| l.0 == slot)?.1 as f64 / 1000.0;
            // The beep before the flash is the same second: −0.5…0.5 s.
            let arrival = ((b - f + 0.5).rem_euclid(1.0) - 0.5) * 1000.0;
            (b - f < 1.0).then_some(Pair {
                at: *f,
                arrival,
                shown: arrival - lead,
                lead,
            })
        })
        .collect()
}

/// Mean of the first and of the last quarter.
fn quarters(values: &[f64]) -> (f64, f64) {
    let quarter = values.len() / 4;
    (
        mean(&values[..quarter]),
        mean(&values[values.len() - quarter..]),
    )
}

/// Prints where the lead of the flash frames changed by a vsync or more.
fn print_lead_changes(pairs: &[Pair]) {
    let mut level = None;
    for p in pairs {
        let now = (p.lead / (1000.0 / FPS)).round();
        if level != Some(now) {
            println!(
                "  {:7.1} s: lead {:5.1} ms, beep − flash {:+6.1} (arrival) {:+6.1} (shown)",
                p.at, p.lead, p.arrival, p.shown
            );
            level = Some(now);
        }
    }
}

#[test]
#[ignore = "records the screen and system sound: needs DEVCAST_TEST_MONITOR"]
fn stays_in_sync() {
    let seconds: u64 = std::env::var("DEVCAST_SYNC_SECONDS").map_or(90, |s| s.parse().unwrap());
    let monitor = test_monitor();
    let (x, y) = (monitor.bounds.x() + 100, monitor.bounds.y() + 100);
    let _flasher = Flasher::show(x, y, 320, 240);
    let _beeps = play_beeps();
    let dir = fresh_dir("sync");
    let config = RecordingConfig {
        fps: Fps::Fps60,
        ..RecordingConfig::new(Source::Area(Area::new(x, y, 320, 240).unwrap()))
    };
    let recording = Recording::start(&config, &dir).unwrap();
    std::thread::sleep(Duration::from_secs(seconds));
    let summary = recording.stop().unwrap();
    let manifest = Manifest::read(&dir).unwrap();
    assert_eq!(manifest.tracks[0].start, manifest.tracks[1].start);
    let flashes = flashes(&summary.files[0]);
    let beeps = beeps(&summary.files[1]);
    let durations = (
        Probe::read(&summary.files[0]).duration,
        summary.audio[0].frames as f64 / f64::from(RATE),
    );
    std::fs::remove_dir_all(&dir).unwrap();

    let pairs = pairs(&flashes, &beeps, &summary.video);
    let arrival: Vec<f64> = pairs.iter().map(|p| p.arrival).collect();
    let shown: Vec<f64> = pairs.iter().map(|p| p.shown).collect();
    let (a_first, a_last) = quarters(&arrival);
    let (s_first, s_last) = quarters(&shown);
    println!(
        "{seconds} s: video {:.3} s, sound {:.3} s; {} flashes, {} beeps, {} pairs
         beep − flash, frame at arrival: mean {:+.1} ms, quarters {a_first:+.1} → {a_last:+.1}
         beep − flash, frame at WGC time: mean {:+.1} ms, quarters {s_first:+.1} → {s_last:+.1}
         frame lead {:?}
sound {:?}",
        durations.0,
        durations.1,
        flashes.len(),
        beeps.len(),
        pairs.len(),
        mean(&arrival),
        mean(&shown),
        summary.video.lead,
        summary.audio[0].stats,
    );
    print_lead_changes(&pairs);
    assert!(
        pairs.len() as u64 >= seconds * 9 / 10,
        "{} pairs",
        pairs.len()
    );
    let frame = 1000.0 / FPS;
    assert!(
        (a_last - a_first).abs() < frame,
        "drifted {a_first} → {a_last}"
    );
}
