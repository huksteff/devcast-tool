//! `discard`, `close`, `save --as`, `rename` and `trash`: on the files without the editor and
//! through the fake editor (`fake_editor.rs`) when the app holds the project.

use dc_ops::live::LOCK_OWNER;
use dc_project::dir::SESSION_FILE;
use dc_project::{ProjectDir, ProjectLock, Working};

use crate::fake_editor::{channel, run};
use crate::live_tests::{pair, working};
use crate::test_util::Fixture;

/// A saved clip and an unsaved one: the working state the editor left in `session.json`.
fn unsaved() -> Fixture {
    let f = Fixture::new();
    assert_eq!(f.run("test add --secs 2").code, 0);
    let mut project = f.dir.load().unwrap();
    project.clips.clear();
    f.dir.save_session(&project).unwrap();
    f
}

#[test]
fn discard_without_the_editor_goes_back_to_project_json() {
    let f = unsaved();
    let out = f.run("discard --dry-run");
    assert!(out.stdout.starts_with("dry-run"), "{}", out.stdout);
    assert!(f.dir.root().join(SESSION_FILE).is_file());
    let out = f.run("discard --json");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["discarded"], true);
    let loaded = Working::load(&f.dir, false).unwrap();
    assert!(!loaded.unsaved);
    assert_eq!(loaded.project.clips.len(), 1);
    assert!(!loaded.history.can_undo(), "the history is cleared");
    let out = f.run("discard");
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("отбрасывать нечего"), "{}", out.stdout);
}

#[test]
fn discard_through_the_editor() {
    let (held, editor, _unlocked) = pair("discard");
    assert_eq!(run(&held, "test rm c1", &editor.channel).code, 0);
    let out = run(&held, "discard", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("отброшены"), "{}", out.stdout);
    let out = run(&held, "discard", &editor.channel);
    assert!(out.stdout.contains("отбрасывать нечего"), "{}", out.stdout);
    let (project, history) = editor.stop();
    assert_eq!(project, held.dir.load().unwrap());
    assert_eq!(project.clips.len(), 2);
    assert!(!history.can_undo());
}

#[test]
fn close_refuses_unsaved_changes_without_a_flag_and_saves_with_save() {
    let (held, editor, unlocked) = pair("close");
    assert_eq!(run(&held, "test rm c1", &editor.channel).code, 0);
    let out = run(&held, "close", &editor.channel);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("добавь --save или --discard"),
        "{}",
        out.stderr
    );
    let out = run(&unlocked, "close --save", &editor.channel);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("в редакторе открыт «Урок 1.devcast»"),
        "{}",
        out.stderr
    );
    let out = run(&held, "close --save --discard", &editor.channel);
    assert_eq!(out.code, 2, "{}", out.stderr);
    let out = run(&held, "close --save", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let (project, _) = editor.gone();
    assert_eq!(held.dir.load().unwrap(), project);
    assert_eq!(project.clips.len(), 1);
    let out = run(&held, "close", &channel("nobody-close"));
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("не запущен"), "{}", out.stderr);
}

#[test]
fn save_as_copies_the_working_state_and_keeps_the_original() {
    let f = unsaved();
    let out = f.run("save --as Копия урока --dry-run");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let copy = f.cwd.join("Копия урока.devcast");
    assert!(!copy.exists());
    std::fs::write(f.dir.media_dir().join("запись.mp4"), b"video").unwrap();
    let out = f.run("save --as Копия урока");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let copy = ProjectDir::open(&copy).unwrap();
    assert!(copy.load().unwrap().clips.is_empty(), "the working state");
    assert!(!copy.has_unsaved_changes());
    assert_eq!(
        std::fs::read(copy.media_dir().join("запись.mp4")).unwrap(),
        b"video"
    );
    assert!(f.dir.has_unsaved_changes(), "the original keeps its state");
    let out = f.run_in("save --as Копия урока", f.dir.root());
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("уже существует"), "{}", out.stderr);
    assert_eq!(f.run("save --open").code, 2);
}

#[test]
fn save_as_through_the_editor_opens_the_copy_there() {
    let (held, editor, _unlocked) = pair("save-as");
    assert_eq!(run(&held, "test rm c1", &editor.channel).code, 0);
    let out = run(&held, "save --as Урок 1 копия --json", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["opened"], true);
    let copy = ProjectDir::open(held.cwd.join("Урок 1 копия.devcast")).unwrap();
    assert_eq!(copy.load().unwrap().clips.len(), 1);
    let (project, _) = editor.stop();
    assert_eq!(
        project.clips.len(),
        2,
        "the editor dropped the working state"
    );
    assert!(!held.dir.has_unsaved_changes());
}

#[test]
fn rename_through_the_editor() {
    let (held, editor, _unlocked) = pair("rename");
    let out = run(&held, "rename Урок новый", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.stdout.trim(), "«Урок 1.devcast» → «Урок новый.devcast»");
    editor.gone();
    assert!(!held.dir.root().exists());
    assert!(
        held.cwd
            .join("Урок новый.devcast")
            .join("project.json")
            .is_file()
    );
}

#[test]
fn trash_refuses_a_held_project_and_dry_run_moves_nothing() {
    let f = Fixture::new();
    let held = ProjectLock::acquire(&f.dir, LOCK_OWNER).unwrap();
    let out = f.run_in("trash .", f.dir.root());
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("devcast close"), "{}", out.stderr);
    drop(held);
    let out = f.run_in("trash . --dry-run --json", f.dir.root());
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["dry_run"], true);
    assert!(f.dir.root().is_dir());
    let out = f.run("trash --restore --dry-run Нет-такого.devcast");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("нет в корзине"), "{}", out.stderr);
}

#[test]
#[ignore = "moves folders through the real Windows Recycle Bin; run at night"]
fn trash_and_restore_through_the_recycle_bin() {
    let f = Fixture::new();
    let out = f.run_in("trash .", f.dir.root());
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!f.dir.root().exists());
    let defaults = crate::test_util::defaults_file(f._tmp.path());
    let local = crate::Local {
        defaults: Some(&defaults),
        ..crate::Local::default()
    };
    let args = ["trash", "--restore", "Урок 1.devcast"]
        .map(str::to_owned)
        .to_vec();
    let out = crate::run_with(args, &f.cwd, &f.registry, local);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(f.dir.root().join("project.json").is_file());
}

#[test]
fn a_fake_editor_of_one_project_is_elsewhere_for_another() {
    let (held, editor, unlocked) = pair("elsewhere");
    drop(ProjectLock::acquire(&unlocked.dir, LOCK_OWNER).unwrap());
    let out = run(&unlocked, "discard", &editor.channel);
    assert_eq!(out.code, 0, "a stale owner name: the files: {}", out.stderr);
    assert_eq!(working(&held).clips.len(), 2);
    editor.stop();
}
