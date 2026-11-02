use std::path::Path;

use dc_core::project::MediaKind;
use dc_project::ProjectDir;

use crate::run;
use crate::test_util::{Recordings, test_video};

struct Videos {
    tmp: tempfile::TempDir,
}

impl Videos {
    /// `Мои видео/демо 1.mp4` (2 s) and `Мои видео/b.mkv` (1 s).
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("Мои видео");
        std::fs::create_dir(&folder).unwrap();
        test_video(&folder.join("демо 1.mp4"), 2.0);
        test_video(&folder.join("b.mkv"), 1.0);
        Self { tmp }
    }

    fn run(&self, args: &[&str]) -> crate::Outcome {
        let args = args.iter().map(|a| (*a).to_owned()).collect();
        run(
            args,
            &self.tmp.path().join("Мои видео"),
            &dc_ops::Registry::devcast().unwrap(),
        )
    }
}

#[test]
fn new_makes_a_project_with_a_clip_per_video() {
    let v = Videos::new();
    let out = v.run(&[
        "new",
        "демо 1.mp4",
        "b.mkv",
        "демо 1.mp4",
        "-o",
        "Урок 3.devcast",
    ]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let dir = ProjectDir::open(v.tmp.path().join("Мои видео").join("Урок 3.devcast")).unwrap();
    assert_eq!(
        out.stdout,
        format!(
            "создан {}: клипы +3 c1..c3; длительность 0:00.000→0:05.000\n",
            dir.root().display()
        )
    );
    let project = dir.load().unwrap();
    let media: Vec<(&str, MediaKind)> = project
        .clips
        .iter()
        .map(|c| (c.media.path.as_str(), c.media.kind))
        .collect();
    assert_eq!(
        media,
        [
            ("media/демо 1.mp4", MediaKind::Imported),
            ("media/b.mkv", MediaKind::Imported),
            ("media/демо 1-2.mp4", MediaKind::Imported),
        ]
    );
    assert!(dir.root().join("media/демо 1-2.mp4").is_file());
    let out = v.run(&["undo", "--project", "Урок 3.devcast"]);
    assert_eq!(out.code, 1, "a new project starts with empty history");
}

#[test]
fn new_dry_run_and_mistakes_leave_no_folder() {
    let v = Videos::new();
    let root = v.tmp.path().join("Мои видео").join("урок.devcast");
    let out = v.run(&["new", "b.mkv", "-o", "урок.devcast", "--dry-run"]);
    assert_eq!(
        out.stdout,
        format!(
            "dry-run, проект не создан: {}: клипы +1 c1; длительность 0:00.000→0:01.000\n",
            root.display()
        )
    );
    let missing = v.run(&["new", "b.mkv", "нет.webm", "-o", "урок.devcast"]);
    assert_eq!(missing.code, 1);
    assert!(missing.stderr.contains("нет.webm"), "{}", missing.stderr);
    std::fs::write(v.tmp.path().join("Мои видео/битый.mov"), "не видео").unwrap();
    let broken = v.run(&["new", "b.mkv", "битый.mov", "-o", "урок.devcast"]);
    assert_eq!(broken.code, 1);
    assert!(broken.stderr.contains("битый.mov"), "{}", broken.stderr);
    assert!(!root.exists());
    assert_eq!(v.run(&["new", "b.mkv", "-o", "урок"]).code, 2);
    assert_eq!(v.run(&["new", "-o", "урок.devcast"]).code, 2);
    std::fs::create_dir(&root).unwrap();
    let taken = v.run(&["new", "b.mkv", "-o", "урок.devcast"]);
    assert_eq!(taken.code, 1);
    assert!(taken.stderr.contains("уже есть"), "{}", taken.stderr);
}

#[test]
fn clip_add_appends_and_undo_takes_it_back() {
    let v = Videos::new();
    let f = Recordings::new();
    let video = v.tmp.path().join("Мои видео/демо 1.mp4");
    let add = |extra: &[&str]| {
        let mut args = vec!["clip".to_owned(), "add".to_owned()];
        args.push(video.to_str().unwrap().to_owned());
        args.extend(extra.iter().map(|a| (*a).to_owned()));
        run(args, f.dir.root(), &dc_ops::Registry::devcast().unwrap())
    };
    let dry = add(&["--dry-run"]);
    assert!(dry.stdout.starts_with("dry-run"), "{}", dry.stdout);
    assert!(!f.dir.media_dir().join("демо 1.mp4").exists());
    assert_eq!(
        add(&[]).stdout,
        "клипы +1 c3; длительность 0:30.000→0:32.000
"
    );
    assert!(f.dir.media_dir().join("демо 1.mp4").is_file());
    assert_eq!(f.ok("clip move c3 --to 1"), "клипы ~1 c3");
    let clips = f.ok("ls clips --fields kind,media");
    assert_eq!(
        clips,
        "c3  0:00.000–0:02.000  kind=imported  media=media/демо 1.mp4\n\
         c1  0:02.000–0:22.000  kind=recording  media=media/a.mp4\n\
         c2  0:22.000–0:32.000  kind=recording  media=media/b.mp4"
    );
    assert_eq!(
        f.ok("undo 2"),
        "отменено: клипы ~1 c3; клипы +1 c3; длительность 0:32.000→0:30.000"
    );
    assert_eq!(f.dir.load().unwrap().clips.len(), 2);
}

#[test]
fn help_shows_clip_add_like_an_op() {
    let f = Recordings::new();
    let help = f.ok("help clip add");
    assert!(
        help.starts_with("clip add — Добавить видео mp4/mov/mkv/webm или записи (папки *.devcast) клипами в конец главной дорожки (копия в media/)\nпример: devcast clip add демо.mp4 ../recording-2.devcast"),
        "{help}"
    );
    let out = f.run("clip add");
    assert_eq!(out.code, 2, "{}", out.stderr);
}

#[test]
fn clip_add_of_a_project_copies_its_recordings_with_telemetry() {
    let source = Recordings::with_telemetry(&[2.0, 1.0]);
    let f = Recordings::with_telemetry(&[3.0]);
    let add = |project: &Path, extra: &[&str]| {
        let mut args = vec!["clip".to_owned(), "add".to_owned()];
        args.push(project.to_str().unwrap().to_owned());
        args.extend(extra.iter().map(|a| (*a).to_owned()));
        run(args, f.dir.root(), &dc_ops::Registry::devcast().unwrap())
    };
    let dry = add(source.dir.root(), &["--dry-run"]);
    assert!(dry.stdout.starts_with("dry-run"), "{}", dry.stderr);
    assert!(!f.dir.media_dir().join("recording-2").exists());
    assert_eq!(
        add(source.dir.root(), &[]).stdout,
        "клипы +2 c2,c3; длительность 0:03.000→0:06.000\n"
    );
    let clips = f.ok("ls clips --fields kind,media");
    assert_eq!(
        clips,
        "c1  0:00.000–0:03.000  kind=recording  media=media/recording-1/video.mp4\n\
         c2  0:03.000–0:05.000  kind=recording  media=media/recording-2/video.mp4\n\
         c3  0:05.000–0:06.000  kind=recording  media=media/recording-3/video.mp4"
    );
    for file in ["manifest.json", "cursor.bin", "keys.bin"] {
        assert!(
            f.dir.media_dir().join("recording-3").join(file).is_file(),
            "{file}"
        );
    }
    let empty = Recordings::of(&[]);
    let out = add(empty.dir.root(), &[]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("нет клипов"), "{}", out.stderr);
    let not_a_project = add(&f.dir.media_dir(), &[]);
    assert_eq!(not_a_project.code, 1);
    assert!(
        not_a_project.stderr.contains("project.json"),
        "{}",
        not_a_project.stderr
    );
    assert!(!f.dir.media_dir().join("recording-4").exists());
}
