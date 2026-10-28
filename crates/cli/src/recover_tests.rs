//! A recording whose process died, without a screen: segments written by the CPU codec into a
//! project made as `devcast record` makes it, left as a killed process leaves them.

use std::path::Path;

use dc_capture::track::TrackWriter;
use dc_capture::{HostTime, Manifest, SharedManifest, Track, VIDEO_TRACK};
use dc_encode::{Codec, Nv12Frame, VideoEncoder, VideoSettings};
use dc_project::{ProjectDir, ProjectLock};

use crate::registry::registry;
use crate::{Outcome, run};

const FPS: u32 = 30;

/// A project with an empty clip list and the recording `media/recording-1` cut off after
/// `frames` frames: the segments of 2 s are complete, the last one is open.
fn killed_recording(root: &Path, frames: i64) -> ProjectDir {
    let dir = ProjectDir::create(root).unwrap();
    let folder = dir.media_dir().join("recording-1");
    let settings = VideoSettings {
        width: 160,
        height: 90,
        fps: FPS,
    };
    let mut encoder = VideoEncoder::open(Codec::X264, settings).unwrap();
    let mut track = Track::new(VIDEO_TRACK, FPS, HostTime::from_micros(42));
    track.size = Some((160, 90));
    let manifest = SharedManifest::create(&folder).unwrap();
    let mut writer = TrackWriter::create(&manifest, track, encoder.parameters()).unwrap();
    let mut frame = Nv12Frame::new(160, 90).unwrap();
    let mut packets = Vec::new();
    for slot in 0..frames {
        frame.fill((slot % 200) as u8, 128, 128);
        encoder.encode(&mut frame, slot, &mut packets).unwrap();
        for packet in packets.drain(..) {
            writer.push(packet).unwrap();
        }
    }
    std::fs::write(folder.join("cursor.bin"), b"DCUR").unwrap();
    // Dropped without `finish`, as a killed process leaves it.
    drop(writer);
    dir
}

fn devcast(dir: &ProjectDir, line: &str) -> Outcome {
    let args = line.split(' ').map(str::to_owned).collect();
    run(args, dir.root(), &registry().unwrap())
}

#[test]
fn opening_the_project_finishes_the_recording_and_adds_its_clip() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = killed_recording(&tmp.path().join("Запись 1.dcproj"), 200);
    let folder = dir.media_dir().join("recording-1");

    let out = devcast(&dir, "info");
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        out.stderr,
        "внимание: восстановлена прерванная запись media/recording-1: клип c1, 0:06.000\n"
    );
    let project = dir.load().unwrap();
    let [clip] = &project.clips[..] else {
        panic!("{:?}", project.clips)
    };
    assert_eq!(clip.media.path, "media/recording-1/video.mp4");
    assert_eq!((clip.media.size.width, clip.media.size.height), (160, 90));
    assert!(Manifest::read(&folder).unwrap().is_finished());
    let mut files: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    assert_eq!(files, ["cursor.bin", "manifest.json", "video.mp4"]);

    let again = devcast(&dir, "info");
    assert_eq!((again.code, again.stderr.as_str()), (0, ""));
    assert_eq!(dir.load().unwrap(), project);
}

#[test]
fn a_recording_without_a_complete_segment_is_deleted() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = killed_recording(&tmp.path().join("Запись 2.dcproj"), 20);
    let out = devcast(&dir, "ls clips");
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stderr.contains("восстанавливать нечего"),
        "{}",
        out.stderr
    );
    assert!(!dir.media_dir().join("recording-1").exists());
    assert!(dir.load().unwrap().clips.is_empty());
}

#[test]
fn a_recording_still_held_by_its_process_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = killed_recording(&tmp.path().join("Запись 3.dcproj"), 200);
    let recording = ProjectLock::acquire(&dir, "devcast record").unwrap();
    let out = devcast(&dir, "info");
    assert_eq!((out.code, out.stderr.as_str()), (0, ""));
    assert!(dir.load().unwrap().clips.is_empty());
    drop(recording);
    assert!(devcast(&dir, "info").stderr.contains("клип c1"));
}

#[test]
fn dry_run_only_reports_the_recording() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = killed_recording(&tmp.path().join("Запись 4.dcproj"), 200);
    let folder = dir.media_dir().join("recording-1");
    let before = std::fs::read(Manifest::path(&folder)).unwrap();
    let out = devcast(&dir, "info --dry-run");
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        out.stderr,
        "внимание: есть прерванная запись media/recording-1, будет восстановлена при открытии без --dry-run\n"
    );
    assert!(dir.load().unwrap().clips.is_empty());
    assert_eq!(std::fs::read(Manifest::path(&folder)).unwrap(), before);
    assert!(!folder.join("video.mp4").exists());
    assert!(!dir.root().join("lock-owner.json").exists());
    assert!(devcast(&dir, "info").stderr.contains("клип c1"));
}
