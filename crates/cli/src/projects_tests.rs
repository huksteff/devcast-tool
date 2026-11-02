//! `devcast ls projects`: the recent projects of the app settings that still exist and the
//! projects of the recordings folder; the running app marks the one its editor holds.

use dc_settings::Store;

use crate::fake_editor::{FakeEditor, channel, run};
use crate::test_util::{Fixture, defaults_file, project_with_clip};

/// `f`'s settings: recordings in `Записи с экрана`, two of them projects with clips, one empty;
/// the recent list has `f`'s project and a folder that is gone.
fn setup(f: &Fixture) -> std::path::PathBuf {
    let folder = f._tmp.path().join("Записи с экрана");
    project_with_clip(&folder.join("Урок 3.devcast"), 2.0);
    project_with_clip(&folder.join("Демо.devcast"), 1.5);
    dc_project::ProjectDir::create(folder.join("Пустой.devcast")).unwrap();
    let file = defaults_file(f._tmp.path()).with_file_name(dc_settings::FILE);
    let mut store = Store::load(&file).unwrap();
    store
        .update(|s| {
            s.recordings_dir = folder.clone();
            s.recent_projects = vec![f.dir.root().to_path_buf(), folder.join("Удалён.devcast")];
        })
        .unwrap();
    folder
}

#[test]
fn recent_and_recordings_without_the_app() {
    let f = Fixture::new();
    let folder = setup(&f);
    let out = run(&f, "ls projects --json", &channel("projects-file"));
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["open"], serde_json::Value::Null);
    let recent = json["recent"].as_array().unwrap();
    assert_eq!(recent.len(), 1, "the folder that is gone is left out");
    assert_eq!(recent[0]["name"], "Урок 1");
    let mut names: Vec<&str> = json["recordings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["Демо", "Урок 3"],
        "a project without clips is not listed"
    );
    assert_eq!(json["recordings_dir"], folder.display().to_string());
    let out = run(&f, "ls projects --from 1", &channel("projects-file"));
    assert_eq!(out.code, 2);
}

#[test]
fn the_app_marks_the_project_of_its_editor() {
    let f = Fixture::new();
    setup(&f);
    let editor = FakeEditor::start(&f, "projects-app");
    let out = run(&f, "ls projects", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let first = out.stdout.lines().nth(1).unwrap();
    assert!(first.ends_with("(открыт в редакторе)"), "{first}");
    assert!(first.starts_with("  Урок 1  "), "{first}");
    editor.stop();
}

#[test]
fn new_without_videos_is_an_empty_project_named_as_in_the_editor() {
    let f = Fixture::new();
    let folder = setup(&f);
    let dead = channel("new-empty");
    let out = run(&f, "new", &dead);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let first = folder.join("Новый проект.devcast");
    assert!(
        out.stdout
            .starts_with(&format!("создан {}", first.display())),
        "{}",
        out.stdout
    );
    let out = run(&f, "new --dry-run", &dead);
    assert!(
        out.stdout.contains("Новый проект 2.devcast"),
        "{}",
        out.stdout
    );
    assert!(!folder.join("Новый проект 2.devcast").exists());
    assert_eq!(run(&f, "new", &dead).code, 0);
    assert!(
        folder
            .join("Новый проект 2.devcast")
            .join("project.json")
            .is_file()
    );
    let out = run(&f, "new -o Черновик.devcast", &dead);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let project = dc_project::ProjectDir::open(f.cwd.join("Черновик.devcast")).unwrap();
    assert!(project.load().unwrap().clips.is_empty());
    assert_eq!(run(&f, "new --each", &dead).code, 2);
    assert_eq!(run(&f, "new видео.mp4", &dead).code, 2);
}

#[test]
fn new_open_asks_the_app_to_open_it() {
    let f = Fixture::new();
    let folder = setup(&f);
    let editor = FakeEditor::start(&f, "new-open");
    let out = run(&f, "new --open", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .ends_with("после ответа о несохранённом в его окне\n"),
        "{}",
        out.stdout
    );
    assert!(folder.join("Новый проект.devcast").is_dir());
    editor.stop();
}
