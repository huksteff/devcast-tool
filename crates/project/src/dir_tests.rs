use dc_core::project::{Clip, Media, MediaKind};
use dc_core::{Range, SourceTime, TimeDelta};

use super::*;

fn with_clip(mut project: Project) -> Project {
    let id = project.clip_ids.allocate();
    project.clips.push(Clip {
        id,
        media: Media {
            path: "media/screen.mp4".to_owned(),
            kind: MediaKind::Recording,
            duration: TimeDelta::from_secs_f64(3.0),
            size: dc_core::PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::from_secs_f64(3.0)).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: dc_core::Rect::FULL,
        audio: Default::default(),
    });
    project
}

fn temp_project() -> (tempfile::TempDir, ProjectDir) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(tmp.path().join("Мой проект с пробелами.dcproj")).unwrap();
    (tmp, dir)
}

#[test]
fn create_then_open_and_load() {
    let (_tmp, dir) = temp_project();
    for folder in ["media", "assets", "cache"] {
        assert!(dir.root().join(folder).is_dir());
    }
    let reopened = ProjectDir::open(dir.root()).unwrap();
    assert_eq!(reopened.load().unwrap(), Project::default());
    let from_inside = ProjectDir::open(dir.media_dir().join("..")).unwrap();
    assert_eq!(from_inside.name(), "Мой проект с пробелами.dcproj");
}

#[test]
fn create_refuses_existing_and_open_refuses_non_projects() {
    let (tmp, dir) = temp_project();
    assert!(matches!(
        ProjectDir::create(dir.root()),
        Err(ProjectError::AlreadyExists { .. })
    ));
    let error = ProjectDir::open(tmp.path()).unwrap_err().to_string();
    assert!(error.contains("не похоже на проект DevCast"), "{error}");
}

#[test]
fn session_holds_unsaved_work_until_save() {
    let (_tmp, dir) = temp_project();
    let edited = with_clip(Project::default());
    dir.save_session(&edited).unwrap();
    assert!(dir.has_unsaved_changes());
    assert_eq!(
        dir.load().unwrap(),
        Project::default(),
        "saved project untouched"
    );
    assert_eq!(dir.load_session().unwrap(), Some(edited.clone()));
    dir.save(&edited).unwrap();
    assert!(!dir.has_unsaved_changes());
    assert_eq!(dir.load().unwrap(), edited);
}

#[test]
fn discard_session_keeps_saved_project() {
    let (_tmp, dir) = temp_project();
    dir.save_session(&with_clip(Project::default())).unwrap();
    dir.discard_session().unwrap();
    dir.discard_session().unwrap();
    assert_eq!(dir.load_session().unwrap(), None);
    assert_eq!(dir.load().unwrap(), Project::default());
}

#[test]
fn save_as_copies_media_and_assets() {
    let (tmp, dir) = temp_project();
    std::fs::write(dir.media_dir().join("screen.mp4"), b"video").unwrap();
    std::fs::create_dir_all(dir.assets_dir().join("fonts")).unwrap();
    std::fs::write(dir.assets_dir().join("fonts").join("Шрифт.ttf"), b"font").unwrap();
    let project = with_clip(Project::default());
    let copy = dir
        .save_as(&project, tmp.path().join("Копия.dcproj"))
        .unwrap();
    assert_eq!(
        std::fs::read(copy.media_dir().join("screen.mp4")).unwrap(),
        b"video"
    );
    assert_eq!(
        std::fs::read(copy.assets_dir().join("fonts").join("Шрифт.ttf")).unwrap(),
        b"font"
    );
    assert_eq!(copy.load().unwrap(), project);
}

#[test]
fn corrupt_and_future_files_explain_what_to_do() {
    let (_tmp, dir) = temp_project();
    std::fs::write(dir.file(PROJECT_FILE), "{ not json").unwrap();
    let error = dir.load().unwrap_err().to_string();
    assert!(
        error.contains("повреждён") && error.contains("session.json"),
        "{error}"
    );
    std::fs::write(
        dir.file(PROJECT_FILE),
        r#"{"schema_version": 99, "clips": [], "clip_ids": 1}"#,
    )
    .unwrap();
    let error = dir.load().unwrap_err().to_string();
    assert!(error.contains("обнови программу"), "{error}");
}

#[test]
fn refuses_to_save_inconsistent_project() {
    let (_tmp, dir) = temp_project();
    let mut broken = with_clip(Project::default());
    broken.clips[0].trim = Range::new(SourceTime::ZERO, SourceTime::from_secs_f64(10.0)).unwrap();
    assert!(matches!(
        dir.save(&broken),
        Err(ProjectError::Inconsistent(_))
    ));
    assert_eq!(dir.load().unwrap(), Project::default());
}
