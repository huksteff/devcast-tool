//! Opening a project in the editor finishes a recording that was never finished, as `devcast`
//! does: without it a recording whose stop failed opened as an empty project.

use dc_capture::input::{CURSOR_FILE, CursorEvent, KEYS_FILE, KeyEvent, RecordWriter, Timed};
use dc_capture::track::TrackWriter;
use dc_capture::{HostTime, SharedManifest, Track, VIDEO_TRACK};
use dc_encode::{Codec, Nv12Frame, VideoEncoder, VideoSettings};
use dc_project::ProjectDir;

use super::Document;

const FPS: u32 = 30;

/// A project without clips and the recording `media/recording-1` cut off after 200 frames, as
/// a killed process or a failed stop leaves it (the same as `recover_tests` of dc-cli).
fn interrupted(root: &std::path::Path) -> ProjectDir {
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
    for slot in 0..200 {
        frame.fill((slot % 200) as u8, 128, 128);
        encoder.encode(&mut frame, slot, &mut packets).unwrap();
        for packet in packets.drain(..) {
            writer.push(packet).unwrap();
        }
    }
    RecordWriter::<Timed<CursorEvent>>::create(&folder.join(CURSOR_FILE))
        .unwrap()
        .flush()
        .unwrap();
    RecordWriter::<Timed<KeyEvent>>::create(&folder.join(KEYS_FILE))
        .unwrap()
        .flush()
        .unwrap();
    drop(writer);
    dir
}

#[test]
fn opening_a_project_brings_its_interrupted_recording_back() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Запись 1.devcast");
    let dir = interrupted(&root);
    assert!(dir.load().unwrap().clips.is_empty());

    let (document, told) = Document::open(dir).unwrap();
    let [clip] = &document.project().clips[..] else {
        panic!("{:?}", document.project().clips);
    };
    assert_eq!(clip.media.path, "media/recording-1/video.mp4");
    assert!(
        told.iter()
            .any(|line| line.contains("восстановлена прерванная запись")),
        "{told:?}"
    );
    drop(document);

    let (again, told) = Document::open(ProjectDir::open(&root).unwrap()).unwrap();
    assert_eq!(again.project().clips.len(), 1);
    assert!(told.is_empty(), "{told:?}");
}
