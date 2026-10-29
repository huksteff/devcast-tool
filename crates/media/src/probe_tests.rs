use dc_core::{PixelSize, SourceTime, TimeDelta};

use super::*;
use crate::test_util::{ffmpeg, speech_video, testsrc, variable_rate};

fn secs(probe: &Probe) -> Vec<f64> {
    probe.keyframes.iter().map(|k| k.as_secs_f64()).collect()
}

#[test]
fn mp4_with_constant_rate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("видео с пробелом.mp4");
    testsrc(&path, "libx264", 30, 30, 3.0);
    let probe = probe(&path).unwrap();
    assert_eq!(
        probe.size,
        PixelSize {
            width: 320,
            height: 240
        }
    );
    assert_eq!(probe.duration, TimeDelta::from_secs_f64(3.0));
    assert_eq!(probe.frames, 90);
    assert!((probe.fps - 30.0).abs() < 1e-9, "{}", probe.fps);
    assert!(!probe.variable_rate);
    assert_eq!(secs(&probe), [0.0, 1.0, 2.0]);
    let kinds: Vec<_> = probe
        .streams
        .iter()
        .map(|s| (s.kind, s.codec.as_str()))
        .collect();
    assert_eq!(kinds, [(StreamKind::Video, "h264")]);
}

#[test]
fn mov_with_short_gop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.mov");
    testsrc(&path, "libx264", 30, 15, 3.0);
    let probe = probe(&path).unwrap();
    assert_eq!(secs(&probe), [0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
    assert_eq!(probe.frames, 90);
}

#[test]
fn mkv_with_b_frames_and_millisecond_time_base() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.mkv");
    testsrc(&path, "libx264", 25, 50, 3.0);
    let probe = probe(&path).unwrap();
    assert_eq!(probe.duration, TimeDelta::from_secs_f64(3.0));
    assert_eq!(probe.frames, 75);
    assert!(!probe.variable_rate);
    assert_eq!(secs(&probe), [0.0, 2.0]);
}

#[test]
fn webm_rounded_to_milliseconds_is_still_constant() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.webm");
    testsrc(&path, "libvpx", 30, 20, 3.0);
    let probe = probe(&path).unwrap();
    assert_eq!(probe.streams[0].codec, "vp8");
    assert_eq!(probe.frames, 90);
    assert_eq!(probe.duration, TimeDelta::from_secs_f64(3.0));
    assert!(!probe.variable_rate);
    assert!(probe.keyframes.len() >= 5, "{:?}", secs(&probe));
}

#[test]
fn variable_rate_is_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vfr.mp4");
    variable_rate(&path);
    let probe = probe(&path).unwrap();
    assert!(probe.variable_rate);
    assert_eq!(probe.frames, 60);
    assert_eq!(probe.duration, TimeDelta::from_secs_f64(3.0));
}

#[test]
fn every_stream_is_listed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sound.mkv");
    ffmpeg(
        "-f lavfi -i sine=duration=2 -f lavfi -i testsrc2=size=160x120:rate=10 -t 2 -c:a flac -c:v libx264",
        &path,
    );
    let probe = probe(&path).unwrap();
    let kinds: Vec<_> = probe
        .streams
        .iter()
        .map(|s| (s.kind, s.codec.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [(StreamKind::Video, "h264"), (StreamKind::Audio, "flac")]
    );
    assert_eq!(probe.video, 0);
}

#[test]
fn our_recording_has_a_keyframe_every_second_and_dropped_frames() {
    let probe = probe(&speech_video()).unwrap();
    assert_eq!(
        probe.size,
        PixelSize {
            width: 1280,
            height: 720
        }
    );
    assert_eq!(probe.duration, TimeDelta::from_micros(20_016_667));
    assert_eq!(probe.frames, 1199);
    assert!(probe.variable_rate);
    for pair in probe.keyframes.windows(2) {
        let gap = pair[1].since(pair[0]).as_secs_f64();
        assert!((1.0..1.1).contains(&gap), "{gap}");
    }
    let t = SourceTime::from_secs_f64(5.5);
    assert!(probe.keyframe_before(t) <= t);
    assert_eq!(
        probe.keyframe_before(SourceTime::from_micros(-1)),
        SourceTime::ZERO
    );
}

#[test]
fn not_a_video() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sound.flac");
    ffmpeg("-f lavfi -i sine=duration=1", &path);
    let error = probe(&path).unwrap_err();
    assert!(matches!(error, MediaError::NoVideo { .. }), "{error}");
    let error = probe(&dir.path().join("нет.mp4")).unwrap_err();
    assert!(error.to_string().contains("нет.mp4"), "{error}");
}

#[test]
fn colour_marks_of_the_stream() {
    let dir = tempfile::tempdir().unwrap();
    let marked = dir.path().join("bt601.mp4");
    ffmpeg(
        "-f lavfi -i testsrc2=size=64x32 -frames:v 1 -pix_fmt yuv420p -colorspace bt470bg -color_range pc -chroma_sample_location center -c:v libx264",
        &marked,
    );
    let color = probe(&marked).unwrap().color;
    assert_eq!(
        color,
        dc_core::VideoColor {
            matrix: dc_core::ColorMatrix::Bt601,
            range: dc_core::ColorRange::Full,
            chroma: dc_core::ChromaSiting::Center,
        }
    );
    let unmarked = dir.path().join("plain.mp4");
    ffmpeg(
        "-f lavfi -i testsrc2=size=64x32 -frames:v 1 -pix_fmt yuv420p -c:v libx264",
        &unmarked,
    );
    assert_eq!(probe(&unmarked).unwrap().color, dc_core::VideoColor::BT709);
}
