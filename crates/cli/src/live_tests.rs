//! `devcast` on a project the editor holds (7.32): a fake editor (`fake_editor.rs`) holds the
//! lock as the app does and keeps the project in memory the way `Document` does.

use dc_core::Project;
use dc_ops::live::{Answer, CHANGED, LOCK_OWNER, Request};
use dc_project::history_log::Event;
use dc_project::{ProjectLock, Working};

use crate::CliError;
use crate::fake_editor::{FakeEditor, channel, run};
use crate::live::Editor;
use crate::test_util::Fixture;

pub(crate) fn working(f: &Fixture) -> Project {
    Working::load(&f.dir, false).unwrap().project
}

/// Two projects with the same history: the editor holds the first, the second is unlocked.
pub(crate) fn pair(case: &str) -> (Fixture, FakeEditor, Fixture) {
    let (held, unlocked) = (Fixture::new(), Fixture::new());
    for f in [&held, &unlocked] {
        assert_eq!(f.run("test add --secs 2").code, 0);
        assert_eq!(f.run("test add --secs 3").code, 0);
    }
    let editor = FakeEditor::start(&held, case);
    (held, editor, unlocked)
}

#[test]
fn a_command_through_the_editor_gives_the_project_of_an_unlocked_copy() {
    let (held, editor, unlocked) = pair("same");
    for line in [
        "test rm c1",
        "test add --secs 1 --dry-run",
        "test rm c9",
        "test add --secs 4 --json",
        "undo",
        "redo",
        "history",
        "test rm --id",
    ] {
        let through = run(&held, line, &editor.channel);
        assert_eq!(through, unlocked.run(line), "{line}");
    }
    let (project, mut history) = editor.stop();
    let copy = Working::load(&unlocked.dir, false).unwrap();
    assert_eq!(project, copy.project);
    assert_eq!(history.changes(), copy.history.changes());
    let mut undone = project.clone();
    history.undo(&mut undone);
    assert_eq!(unlocked.run("undo").code, 0);
    assert_eq!(
        undone,
        working(&unlocked),
        "undo in the editor reverts the CLI command"
    );
}

#[test]
fn reading_commands_see_what_the_editor_has_not_written_yet() {
    let (held, editor, unlocked) = pair("read");
    assert_eq!(run(&held, "test rm c2", &editor.channel).code, 0);
    assert_eq!(unlocked.run("test rm c2").code, 0);
    assert_eq!(
        run(&held, "history", &editor.channel),
        unlocked.run("history")
    );
    assert_eq!(working(&held), working(&unlocked));
    let saved = run(&held, "save", &editor.channel);
    assert_eq!(
        saved.stdout,
        "сохранено в project.json
",
        "{}",
        saved.stderr
    );
    let (project, _) = editor.stop();
    assert_eq!(
        held.dir.load().unwrap(),
        project,
        "saved through the editor"
    );
}

#[test]
fn a_write_over_a_state_the_editor_changed_meanwhile_is_refused() {
    let (held, editor, _unlocked) = pair("changed");
    let mut cli_run = Editor::open(&held.dir, Some(&editor.channel))
        .unwrap()
        .unwrap();
    let other = Request::Write {
        root: held.dir.root().to_path_buf(),
        revision: 0,
        events: vec![Event::Undo],
    };
    let written: Answer = dc_ipc::request(&editor.channel, &other).unwrap();
    assert_eq!(written, Answer::Written { revision: 1 });
    let error = cli_run.write(&[Event::Redo]).unwrap_err();
    assert_eq!(error, CliError::Failed(CHANGED.to_owned()));
    let (project, history) = editor.stop();
    assert_eq!(project.clips.len(), 1, "only the first write was taken");
    assert!(history.can_redo());
}

#[test]
fn without_a_running_app_the_cli_edits_the_files() {
    let f = Fixture::new();
    drop(ProjectLock::acquire(&f.dir, LOCK_OWNER).unwrap());
    let nobody = channel("nobody");
    let out = run(&f, "test add --secs 2", &nobody);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        f.clips(),
        1,
        "lock-owner.json names the app, yet it is gone"
    );
    let _held = ProjectLock::acquire(&f.dir, LOCK_OWNER).unwrap();
    let out = run(&f, "test add --secs 2", &nobody);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("проект уже открыт"), "{}", out.stderr);
}

#[test]
fn open_asks_the_app_to_show_the_project_in_its_editor() {
    let (held, editor, unlocked) = pair("open");
    let out = run(&held, "open", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("открыт в редакторе"), "{}", out.stdout);
    let out = run(&unlocked, "open", &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("спрашивает"), "{}", out.stdout);
    editor.stop();
    let out = run(&held, "open", &channel("nobody-open"));
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("не запущен"), "{}", out.stderr);
}

#[test]
fn a_font_added_through_the_editor_reaches_its_font_lists() {
    let (held, editor, _unlocked) = pair("font");
    let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/PressStart2P-Regular.ttf");
    let line = format!("font add {}", font.display());
    let dry = run(&held, &format!("{line} --dry-run"), &editor.channel);
    assert_eq!(dry.code, 0, "{}", dry.stderr);
    assert!(
        editor.fonts.lock().unwrap().is_empty(),
        "a dry run adds nothing"
    );
    let out = run(&held, &line, &editor.channel);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let heard = editor.fonts.lock().unwrap().clone();
    assert_eq!(heard.len(), 1);
    assert!(heard[0].iter().any(|f| f == "Press Start 2P"), "{heard:?}");
    editor.stop();
}
