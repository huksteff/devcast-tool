use dc_project::ProjectDir;

use super::*;
use crate::test_util::testsrc;

fn project_with_video() -> (tempfile::TempDir, ProjectDir) {
    let tmp = tempfile::tempdir().unwrap();
    let project = ProjectDir::create(tmp.path().join("Мой урок.dcproj")).unwrap();
    std::fs::create_dir(project.media_dir().join("import")).unwrap();
    testsrc(
        &project.root().join("media/import/клип 1.mp4"),
        "libx264",
        30,
        30,
        2.0,
    );
    (tmp, project)
}

const MEDIA: &str = "media/import/клип 1.mp4";

#[test]
fn cache_lies_under_the_media_path() {
    let (_tmp, project) = project_with_video();
    let path = probe_cache_path(&project, MEDIA);
    assert_eq!(
        path,
        project
            .cache_dir()
            .join("media")
            .join("import")
            .join("клип 1.mp4.probe.json")
    );
}

#[test]
fn second_probe_comes_from_the_cache() {
    let (_tmp, project) = project_with_video();
    let first = cached_probe(&project, MEDIA).unwrap();
    assert_eq!(first.frames, 60);
    let cache = probe_cache_path(&project, MEDIA);
    let text = std::fs::read_to_string(&cache).unwrap();
    std::fs::write(&cache, text.replace("\"frames\": 60", "\"frames\": 7")).unwrap();
    assert_eq!(cached_probe(&project, MEDIA).unwrap().frames, 7);
}

#[test]
fn changed_file_is_probed_again() {
    let (_tmp, project) = project_with_video();
    cached_probe(&project, MEDIA).unwrap();
    testsrc(&project.root().join(MEDIA), "libx264", 30, 30, 1.0);
    assert_eq!(cached_probe(&project, MEDIA).unwrap().frames, 30);
}

#[test]
fn broken_cache_is_built_again() {
    let (_tmp, project) = project_with_video();
    let cache = probe_cache_path(&project, MEDIA);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, "{").unwrap();
    assert_eq!(cached_probe(&project, MEDIA).unwrap().frames, 60);
    assert!(
        std::fs::read_to_string(&cache)
            .unwrap()
            .contains("\"frames\": 60")
    );
}

#[test]
fn missing_media_is_an_error_with_its_path() {
    let (_tmp, project) = project_with_video();
    let error = cached_probe(&project, "media/нет.mp4").unwrap_err();
    assert!(error.to_string().contains("нет.mp4"), "{error}");
}
