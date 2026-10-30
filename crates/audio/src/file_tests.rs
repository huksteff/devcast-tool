use std::path::Path;

use dc_core::TimeDelta;
use dc_project::ProjectDir;

use super::*;
use crate::test_util::ffmpeg;

/// Makes `name` in `dir`: a 440 Hz tone `secs` long, coded by the extension.
fn tone(dir: &Path, name: &str, secs: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let source = format!("sine=f=440:r=44100:d={secs}");
    ffmpeg(&["-f", "lavfi", "-i", &source, path.to_str().unwrap()]);
    path
}

#[test]
fn every_format_of_scope_reads_from_its_first_sample_with_its_length() {
    let dir = tempfile::tempdir().unwrap();
    for extension in AUDIO_EXTENSIONS {
        let path = tone(dir.path(), &format!("тон 1.{extension}"), "2.5");
        let track = file_track(&path).unwrap();
        let duration = file_duration(&track).unwrap();
        // Raw ADTS (`.aac`) does not say how many samples of coder priming lead the sound, so
        // they play, as in any player: 1024 samples of the file's rate, ~23 ms here, and the
        // padding of the last frame. The other formats are exact.
        let (late, longer) = if extension == "aac" {
            (1200, 0.04)
        } else {
            (48, 0.001)
        };
        let off = duration.as_secs_f64() - 2.5;
        assert!((0.0..longer).contains(&off), "{extension}: {duration:?}");
        let mut head = vec![0.0; 2048 * CHANNELS];
        TrackReader::open(&track)
            .unwrap()
            .fill(0, &mut head)
            .unwrap();
        let first_loud = head.iter().position(|s| s.abs() > 0.01).unwrap() / CHANNELS;
        assert!(
            first_loud < late,
            "{extension}: sound starts at frame {first_loud}"
        );
    }
}

#[test]
fn a_file_without_sound_or_of_another_kind_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let project = ProjectDir::create(dir.path().join("p.dcproj")).unwrap();
    let video = dir.path().join("кадр.mp4");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=16x16:d=1",
        video.to_str().unwrap(),
    ]);
    let error = file_track(&video).unwrap_err().to_string();
    assert!(error.contains("нет звука"), "{error}");
    let error = import_sound(&project, "assets/audio", &video, false).unwrap_err();
    assert!(error.to_string().contains("mp3, wav"), "{error}");
    let missing = dir.path().join("нет.mp3");
    let error = import_sound(&project, "assets/audio", &missing, false).unwrap_err();
    assert!(error.to_string().contains("нет файла"), "{error}");
}

#[test]
fn import_copies_once_and_a_dry_run_copies_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let project = ProjectDir::create(dir.path().join("Урок.dcproj")).unwrap();
    let music = tone(dir.path(), "Музыка.MP3", "1");
    let dry = import_sound(&project, "assets/audio", &music, true).unwrap();
    assert_eq!(
        (dry.path.as_str(), dry.copied),
        ("assets/audio/Музыка.mp3", false)
    );
    assert!(!project.assets_dir().join("audio").exists());
    let first = import_sound(&project, "assets/audio", &music, false).unwrap();
    assert!(first.copied);
    assert!(first.duration > TimeDelta::from_millis(990));
    let again = import_sound(&project, "assets/audio", &music, false).unwrap();
    assert_eq!((again.path, again.copied), (first.path, false));
}
