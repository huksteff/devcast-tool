use std::path::Path;

use dc_core::SourceTime;

use super::*;
use crate::probe::probe;
use crate::seeker::{Precision, Seeker};
use crate::test_util::{frame_times, reference_frames, testsrc, variable_rate};

const MB: usize = 1_000_000;

/// Deterministic pseudo-random numbers below `n` (a linear congruential generator).
fn random(seed: u64, count: usize, n: usize) -> Vec<usize> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) % n as u64) as usize
        })
        .collect()
}

fn open(path: &Path, limit: usize) -> Decoder {
    Decoder::open(path, probe(path).unwrap(), FramePool::new(limit)).unwrap()
}

fn bytes(frame: &Frame) -> Vec<u8> {
    [frame.y(), frame.uv()].concat()
}

/// Seeks to the middle of 50 random frames in random order (back and forth, inside one GOP and
/// across) and compares each with FFmpeg's frame of that number; frame start times are
/// ffprobe's.
fn exact_at_random_frames(path: &Path) {
    let reference = reference_frames(path);
    let starts = frame_times(path);
    let mut decoder = open(path, 16 * MB);
    let len = nv12_len(decoder.probe().size);
    assert_eq!(reference.len(), starts.len() * len, "{}", path.display());
    for k in random(path.as_os_str().len() as u64, 50, starts.len()) {
        let end = starts.get(k + 1).copied().unwrap_or(starts[k] + 0.01);
        let t = SourceTime::from_secs_f64((starts[k] + end) / 2.0);
        let frame = decoder.frame_at(t).unwrap();
        assert!(
            (frame.time.as_secs_f64() - starts[k]).abs() < 0.001,
            "{}: frame {k} at {}",
            path.display(),
            frame.time
        );
        assert!(
            bytes(&frame) == reference[k * len..(k + 1) * len],
            "{}: frame {k} differs from FFmpeg's",
            path.display()
        );
    }
}

#[test]
fn exact_frame_in_every_container() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("gop 1s.mp4", "libx264", 30, 30),
        ("b frames gop 2s.mkv", "libx264", 25, 50),
        ("short gop.mov", "libx264", 30, 10),
        ("vp8.webm", "libvpx", 30, 20),
    ];
    for (name, codec, fps, gop) in cases {
        let path = dir.path().join(name);
        testsrc(&path, codec, fps, gop, 4.0);
        exact_at_random_frames(&path);
    }
}

#[test]
fn exact_frame_with_variable_rate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vfr.mp4");
    variable_rate(&path);
    assert!(probe(&path).unwrap().variable_rate);
    exact_at_random_frames(&path);
}

#[test]
fn ends_repeats_and_keyframes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.mp4");
    testsrc(&path, "libx264", 30, 30, 2.0);
    let reference = reference_frames(&path);
    let mut decoder = open(&path, 16 * MB);
    let len = nv12_len(decoder.probe().size);
    let first = decoder.frame_at(SourceTime::from_micros(-5)).unwrap();
    assert_eq!(bytes(&first), reference[..len]);
    let last = decoder.frame_at(SourceTime::from_secs_f64(9.0)).unwrap();
    assert_eq!(bytes(&last), reference[59 * len..]);
    let at = decoder.frame_at(SourceTime::from_secs_f64(1.5)).unwrap();
    let again = decoder.frame_at(at.time).unwrap();
    assert_eq!((again.time, bytes(&again)), (at.time, bytes(&at)));
    let key = decoder.keyframe_at(SourceTime::from_secs_f64(1.9)).unwrap();
    assert_eq!(key.time, SourceTime::from_secs_f64(1.0));
    assert_eq!(bytes(&key), reference[30 * len..31 * len]);
}

#[test]
fn the_pool_limit_holds_while_frames_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.mkv");
    testsrc(&path, "libx264", 30, 30, 1.0);
    let len = nv12_len(probe(&path).unwrap().size);
    let mut decoder = open(&path, 2 * len);
    let a = decoder.frame_at(SourceTime::from_secs_f64(0.1)).unwrap();
    let b = decoder.frame_at(SourceTime::from_secs_f64(0.2)).unwrap();
    let error = decoder
        .frame_at(SourceTime::from_secs_f64(0.3))
        .unwrap_err();
    assert!(matches!(error, MediaError::PoolFull { .. }), "{error}");
    drop((a, b));
    decoder.frame_at(SourceTime::from_secs_f64(0.3)).unwrap();
}

#[test]
fn stale_seeks_never_show() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.webm");
    testsrc(&path, "libvpx", 30, 30, 3.0);
    let reference = reference_frames(&path);
    let probe = probe(&path).unwrap();
    let len = nv12_len(probe.size);
    let pool = FramePool::new(64 * MB);
    let mut seeker = Seeker::start(&path, probe, pool.clone());
    let mut newest = 0;
    for k in random(7, 40, 90) {
        newest = k;
        let t = SourceTime::from_secs_f64((k as f64 + 0.5) / 30.0);
        seeker.seek(t, Precision::Exact).unwrap();
        if let Some(frame) = seeker.poll().unwrap() {
            assert_eq!(bytes(&frame), reference[k * len..(k + 1) * len]);
        }
    }
    let frame = seeker.wait().unwrap();
    assert_eq!(bytes(&frame), reference[newest * len..(newest + 1) * len]);
    assert!(pool.peak() <= 4 * len, "{} bytes", pool.peak());
}

#[test]
fn a_missing_file_answers_the_first_seek() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.mp4");
    testsrc(&path, "libx264", 30, 30, 1.0);
    let probe = probe(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut seeker = Seeker::start(&path, probe, FramePool::new(MB));
    seeker.seek(SourceTime::ZERO, Precision::Exact).unwrap();
    let error = seeker.wait().unwrap_err();
    assert!(matches!(error, MediaError::Open { .. }), "{error}");
}

/// Seek timing for docs (decision 003): `cargo nextest run -p dc-media --run-ignored only
/// seek_timing --no-capture`. 100 exact and 100 keyframe seeks at random times.
#[test]
#[ignore = "timing on this PC; run by hand"]
fn seek_timing() {
    let dir = tempfile::tempdir().unwrap();
    let synthetic = dir.path().join("1080p60 gop 1s.mp4");
    crate::test_util::ffmpeg(
        "-f lavfi -i testsrc2=size=1920x1080:rate=60 -t 10 -c:v libx264 -g 60 -pix_fmt yuv420p",
        &synthetic,
    );
    for path in [crate::test_util::speech_video(), synthetic] {
        let probe = probe(&path).unwrap();
        let pool = FramePool::new(64 * MB);
        let mut decoder = Decoder::open(&path, probe.clone(), pool.clone()).unwrap();
        let micros = probe.duration.micros() as usize;
        for precision in [Precision::Exact, Precision::Keyframe] {
            let mut times: Vec<f64> = random(42, 100, micros)
                .into_iter()
                .map(|us| {
                    let t = SourceTime::from_micros(us as i64);
                    let started = std::time::Instant::now();
                    match precision {
                        Precision::Exact => decoder.frame_at(t),
                        Precision::Keyframe => decoder.keyframe_at(t),
                    }
                    .unwrap();
                    started.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            times.sort_by(f64::total_cmp);
            println!(
                "{} {}x{} {precision:?}: mean {:.1} ms, p95 {:.1} ms, max {:.1} ms; pool peak {:.1} MB",
                path.file_name().unwrap().to_string_lossy(),
                probe.size.width,
                probe.size.height,
                times.iter().sum::<f64>() / times.len() as f64,
                times[94],
                times[99],
                pool.peak() as f64 / MB as f64
            );
        }
    }
}
