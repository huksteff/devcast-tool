use std::sync::mpsc;

use dc_encode::{Codec, VideoSettings};

use super::*;
use crate::config::Fps;
use crate::manifest::{Manifest, SharedManifest, Track};

const FPS: u32 = 30;

fn at(secs: f64) -> HostTime {
    HostTime::from_micros(1_000_000_000 + (secs * 1_000_000.0) as i64)
}

/// An encoder state writing into `dir`, with one picture encoded at slot 0.
fn started(dir: &std::path::Path) -> (Encoding, FrameGrid, mpsc::Receiver<Nv12Frame>) {
    let settings = VideoSettings {
        width: 160,
        height: 90,
        fps: FPS,
    };
    let encoder = VideoEncoder::open(Codec::X264, settings).unwrap();
    let manifest = SharedManifest::create(dir).unwrap();
    let track = Track::new("video", FPS, at(0.0));
    let track = TrackWriter::create(&manifest, track, encoder.parameters()).unwrap();
    let mut encoding = Encoding {
        encoder,
        track,
        packets: Vec::new(),
        count: 0,
        leads: Vec::new(),
        last: None,
        kept: None,
    };
    let (returned, back) = mpsc::channel();
    let mut frame = Nv12Frame::new(settings.width, settings.height).unwrap();
    frame.fill(100, 128, 128);
    encoding.put(frame, 0, 0, &returned).unwrap();
    let grid = FrameGrid::new(at(0.0), Fps::try_from(FPS).unwrap());
    (encoding, grid, back)
}

fn segments(dir: &std::path::Path) -> usize {
    Manifest::read(dir).unwrap().tracks[0].segments.len()
}

#[test]
fn a_still_screen_repeats_the_last_picture_and_closes_segments() {
    let dir = tempfile::tempdir().unwrap();
    let (mut encoding, grid, _back) = started(dir.path());
    let clock = PauseClock::default();
    assert_eq!(
        segments(dir.path()),
        0,
        "no frames after the first one: nothing closed"
    );
    // x264 (veryfast) hands out packets 14 frames late, so a still screen closes its first
    // segment after ~16 repeats; then one every 2 s.
    for second in 1..=20 {
        encoding
            .repeat(&grid, &clock, at(f64::from(second)))
            .unwrap();
    }
    assert_eq!(encoding.count, 21);
    assert_eq!(encoding.last, Some(20 * i64::from(FPS)));
    assert!(
        segments(dir.path()) >= 2,
        "repeats are key frames that close segments"
    );
    assert_eq!(encoding.leads.len(), 1, "repeats are not capture leads");
}

#[test]
fn no_repeat_during_a_pause_or_at_the_slot_of_the_last_frame() {
    let dir = tempfile::tempdir().unwrap();
    let (mut encoding, grid, _back) = started(dir.path());
    let clock = PauseClock::default();
    encoding.repeat(&grid, &clock, at(0.01)).unwrap();
    assert_eq!(encoding.count, 1, "slot 0 is taken already");
    clock.pause(at(0.5)).unwrap();
    encoding.repeat(&grid, &clock, at(1.5)).unwrap();
    assert_eq!(encoding.count, 1, "paused");
    assert!(
        encoding.kept.is_some(),
        "the picture stays for after the pause"
    );
    clock.resume(at(2.5)).unwrap();
    encoding.repeat(&grid, &clock, at(3.5)).unwrap();
    assert_eq!(encoding.count, 2);
    assert_eq!(
        encoding.last,
        Some(i64::from(FPS) * 3 / 2),
        "the pause is cut out"
    );
}

#[test]
fn the_picture_before_the_last_goes_back_to_capture() {
    let dir = tempfile::tempdir().unwrap();
    let (mut encoding, _grid, back) = started(dir.path());
    assert!(back.try_recv().is_err(), "the only picture is kept");
    let (returned, again) = mpsc::channel();
    let frame = Nv12Frame::new(160, 90).unwrap();
    encoding.put(frame, 1, 0, &returned).unwrap();
    assert!(again.try_recv().is_ok(), "the first picture went back");
    assert!(encoding.kept.is_some(), "the second one is kept");
}
