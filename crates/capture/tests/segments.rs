//! Segmented writing and finalizing without a screen: frames from the CPU codec.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use std::path::Path;

use dc_capture::track::TrackWriter;
use dc_capture::{HostTime, Manifest, SharedManifest, Track, finalize, recover};
use dc_encode::{Codec, Mp4Reader, Nv12Frame, VideoEncoder, VideoSettings};

const FPS: u32 = 30;

/// Encodes frames at `slots` (a moving dot, so frames differ a little) into a track writer in
/// `dir`; returns the manifest after `finish(end)`.
fn write_track(dir: &Path, slots: &[i64], end: i64) -> Manifest {
    let writer = write_frames(dir, slots);
    writer.finish(end).unwrap();
    Manifest::read(dir).unwrap()
}

/// Encodes and pushes frames at `slots` without finishing: the last segment stays open.
fn write_frames(dir: &Path, slots: &[i64]) -> TrackWriter {
    let settings = VideoSettings {
        width: 160,
        height: 90,
        fps: FPS,
    };
    let mut encoder = VideoEncoder::open(Codec::X264, settings).unwrap();
    let track = Track::new("video", FPS, HostTime::from_micros(42));
    let manifest = SharedManifest::create(dir).unwrap();
    let mut writer = TrackWriter::create(&manifest, track, encoder.parameters()).unwrap();
    let mut frame = Nv12Frame::new(settings.width, settings.height).unwrap();
    let mut packets = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        frame.fill(100, 128, 128);
        if let Some(row) = frame.luma_rows_mut().nth(i % 90) {
            row[i % 160] = 200;
        }
        encoder.encode(&mut frame, *slot, &mut packets).unwrap();
        for packet in packets.drain(..) {
            writer.push(packet).unwrap();
        }
    }
    encoder.finish(&mut packets).unwrap();
    for packet in packets.drain(..) {
        writer.push(packet).unwrap();
    }
    writer
}

fn read_all(path: &Path) -> Vec<(i64, i64, bool)> {
    let mut reader = Mp4Reader::open(path, (1, FPS as i32)).unwrap();
    let mut packets = Vec::new();
    while let Some(p) = reader.read().unwrap() {
        packets.push((p.pts(), p.duration(), p.is_key()));
    }
    packets
}

#[test]
fn segments_cover_the_track_and_join_into_one_file() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().join("Запись 1");
    // 7 s at 30 fps with a still screen from 3.0 to 4.5 s.
    let slots: Vec<i64> = (0..90).chain(135..210).collect();
    let mut manifest = write_track(&dir, &slots, 215);

    let track = &manifest.tracks[0];
    let bounds: Vec<(i64, i64)> = track.segments.iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(bounds, [(0, 60), (60, 135), (135, 195), (195, 215)]);
    for segment in &track.segments {
        let packets = read_all(&dir.join(&segment.file));
        assert!(packets[0].2, "{} starts without a key frame", segment.file);
        assert_eq!(packets[0].0, 0, "{} times start from 0", segment.file);
        let last = packets[packets.len() - 1];
        assert_eq!(segment.start + last.0 + last.1, segment.end);
    }

    let files = finalize(&dir, &mut manifest).unwrap();
    assert_eq!(files, [dir.join("video.mp4")]);
    let packets = read_all(&files[0]);
    let times: Vec<i64> = packets.iter().map(|p| p.0).collect();
    assert_eq!(times, slots);
    assert_eq!(
        packets[89].1, 46,
        "the frame before the still screen lasts through it"
    );
    assert_eq!(
        packets[packets.len() - 1].1,
        6,
        "the last frame lasts until the stop"
    );
    let keys: Vec<i64> = packets.iter().filter(|p| p.2).map(|p| p.0).collect();
    // x264 may add key frames of its own at scene changes.
    for required in [0, 30, 60, 135, 165, 195] {
        assert!(keys.contains(&required), "{keys:?}");
    }

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["manifest.json", "video.mp4"]);
    let finished = Manifest::read(&dir).unwrap();
    assert!(finished.is_finished());
    assert!(finished.tracks[0].segments.is_empty());
}

/// The process died: the open segment is half written and not in the manifest, a manifest
/// replacement was cut off. Recovery keeps the complete segments only.
#[test]
fn recovery_keeps_complete_segments_and_drops_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().join("вылет");
    let slots: Vec<i64> = (0..200).collect();
    // Dropped without `finish`: files are closed (as the system does for a killed process)
    // but the open segment is not completed and not listed.
    drop(write_frames(&dir, &slots));
    let open = dir.join("video-000004.mp4");
    let bytes = std::fs::read(&open).unwrap();
    std::fs::write(&open, &bytes[..bytes.len() / 2]).unwrap();
    std::fs::write(dir.join("manifest.json.tmp"), "{\"vers").unwrap();
    std::fs::write(dir.join("notes.txt"), "чужой файл").unwrap();
    let before = Manifest::read(&dir).unwrap();
    assert_eq!(before.tracks[0].end(), 180);

    let files = recover(&dir).unwrap();
    let packets = read_all(&files[0]);
    let times: Vec<i64> = packets.iter().map(|p| p.0).collect();
    assert_eq!(times, (0..180).collect::<Vec<_>>());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["manifest.json", "notes.txt", "video.mp4"]);
    assert!(Manifest::read(&dir).unwrap().is_finished());
    // Running it again on a finished recording changes nothing.
    assert_eq!(recover(&dir).unwrap(), files);
    assert_eq!(read_all(&files[0]), packets);
}

/// The microphone sent nothing that closed a segment: the recording keeps its video and the
/// empty track goes away with its half-written segment.
#[test]
fn a_sound_track_without_segments_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().join("без звука");
    let mut manifest = write_track(&dir, &(0..90).collect::<Vec<_>>(), 90);
    manifest
        .tracks
        .push(Track::new("mic", 48_000, HostTime::from_micros(42)));
    std::fs::write(dir.join("mic-000001.mp4"), "оборван").unwrap();

    let files = finalize(&dir, &mut manifest).unwrap();
    assert_eq!(files, [dir.join("video.mp4")]);
    let finished = Manifest::read(&dir).unwrap();
    assert_eq!(finished.tracks.len(), 1);
    assert!(finished.is_finished());
    assert!(!dir.join("mic-000001.mp4").exists());
}

#[test]
fn a_video_track_without_segments_is_an_empty_recording() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().join("пусто");
    let mut manifest = write_track(&dir, &[], 0);
    assert!(matches!(
        finalize(&dir, &mut manifest),
        Err(dc_capture::CaptureError::EmptyRecording(_))
    ));
}
