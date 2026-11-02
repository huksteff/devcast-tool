use std::path::{Path, PathBuf};

use dc_core::Project;
use dc_core::project::MediaKind;
use dc_project::ProjectDir;

use super::{ClipImportError, import_clips};

fn clicks() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.devcast")
}

fn target(tmp: &Path) -> ProjectDir {
    let dir = ProjectDir::create(tmp.join("Урок 1.devcast")).unwrap();
    dir.save(&Project::default()).unwrap();
    dir
}

#[test]
fn a_project_gives_its_recording_folders_as_recordings() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = target(tmp.path());
    let import = import_clips(&dir, &[clicks(), clicks()], false).unwrap();
    let media: Vec<(&str, MediaKind)> = import
        .commands
        .iter()
        .map(|c| (c.media.path.as_str(), c.media.kind))
        .collect();
    assert_eq!(
        media,
        [
            ("media/recording-1/video.mp4", MediaKind::Recording),
            ("media/recording-2/video.mp4", MediaKind::Recording),
        ]
    );
    let source = clicks().join("media/recording-1");
    for file in [
        "video.mp4",
        "system.mp4",
        "cursor.bin",
        "keys.bin",
        "manifest.json",
    ] {
        let copy = dir.media_dir().join("recording-2").join(file);
        assert_eq!(
            std::fs::read(copy).unwrap(),
            std::fs::read(source.join(file)).unwrap()
        );
    }
    import.discard().unwrap();
    assert!(!dir.media_dir().join("recording-1").exists());
    assert!(!dir.media_dir().join("recording-2").exists());
}

#[test]
fn a_dry_run_copies_nothing_and_names_the_same() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = target(tmp.path());
    let import = import_clips(&dir, &[clicks(), clicks()], true).unwrap();
    assert_eq!(import.commands[1].media.path, "media/recording-2/video.mp4");
    assert!(!dir.media_dir().join("recording-1").exists());
}

#[test]
fn a_mistake_keeps_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = target(tmp.path());
    let empty = ProjectDir::create(tmp.path().join("пусто.devcast")).unwrap();
    empty.save(&Project::default()).unwrap();
    let error = import_clips(&dir, &[clicks(), empty.root().to_path_buf()], false).unwrap_err();
    assert!(matches!(error, ClipImportError::Empty(_)), "{error}");
    assert!(!dir.media_dir().join("recording-1").exists());
    let folder = tmp.path().join("папка");
    std::fs::create_dir(&folder).unwrap();
    let error = import_clips(&dir, &[folder], false).unwrap_err();
    assert!(error.to_string().contains("project.json"), "{error}");
}
