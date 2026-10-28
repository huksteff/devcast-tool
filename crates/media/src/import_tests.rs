use std::path::PathBuf;

use dc_core::TimeDelta;
use dc_core::project::MediaKind;
use dc_project::ProjectDir;

use super::*;
use crate::test_util::testsrc;

struct Setup {
    _tmp: tempfile::TempDir,
    project: ProjectDir,
    videos: PathBuf,
}

fn setup() -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let project = ProjectDir::create(tmp.path().join("Урок.dcproj")).unwrap();
    let videos = tmp.path().join("Мои видео");
    std::fs::create_dir(&videos).unwrap();
    testsrc(&videos.join("демо 1.mp4"), "libx264", 30, 30, 2.0);
    testsrc(&videos.join("b.webm"), "libvpx", 25, 25, 1.0);
    Setup {
        _tmp: tmp,
        project,
        videos,
    }
}

#[test]
fn videos_are_copied_under_free_names_and_probed() {
    let s = setup();
    let sources = [
        s.videos.join("демо 1.mp4"),
        s.videos.join("b.webm"),
        s.videos.join("демо 1.mp4"),
    ];
    let media = import(&s.project, &sources).unwrap();
    let paths: Vec<&str> = media.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(
        paths,
        ["media/демо 1.mp4", "media/b.webm", "media/демо 1-2.mp4"]
    );
    assert!(media.iter().all(|m| m.kind == MediaKind::Imported));
    assert_eq!(media[0].duration, TimeDelta::from_secs_f64(2.0));
    assert_eq!(media[1].duration, TimeDelta::from_secs_f64(1.0));
    assert_eq!((media[0].size.width, media[0].size.height), (320, 240));
    for m in &media {
        assert!(s.project.root().join(&m.path).is_file());
        assert!(crate::probe_cache_path(&s.project, &m.path).is_file());
    }
    let again = import(&s.project, &[s.videos.join("b.webm")]).unwrap();
    assert_eq!(again[0].path, "media/b-2.webm");
}

#[test]
fn dry_run_copies_nothing_and_names_the_same() {
    let s = setup();
    let sources = [s.videos.join("демо 1.mp4"), s.videos.join("демо 1.mp4")];
    let media = import_dry_run(&s.project.media_dir(), &sources).unwrap();
    assert_eq!(media[1].path, "media/демо 1-2.mp4");
    assert_eq!(media[1].duration, TimeDelta::from_secs_f64(2.0));
    assert_eq!(std::fs::read_dir(s.project.media_dir()).unwrap().count(), 0);
}

#[test]
fn a_bad_source_leaves_the_project_as_it_was() {
    let s = setup();
    let text = s.videos.join("заметки.txt");
    std::fs::write(&text, "не видео").unwrap();
    let broken = s.videos.join("broken.mkv");
    std::fs::write(&broken, "не видео").unwrap();
    let good = s.videos.join("b.webm");
    for (bad, message) in [
        (text, "mp4, mov, mkv и webm"),
        (s.videos.join("нет.mov"), "не найден"),
        (broken, "broken.mkv"),
    ] {
        let error = import(&s.project, &[good.clone(), bad]).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert_eq!(std::fs::read_dir(s.project.media_dir()).unwrap().count(), 0);
    }
}
