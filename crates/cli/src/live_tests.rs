//! `devcast` on a project the editor holds (7.32): a fake editor on its own channel holds the
//! lock as the app does and keeps the project in memory the way `Document` does (writes the
//! working state on `Open`, takes events with `dc_ops::live::replay`).

use std::thread::JoinHandle;

use dc_core::{History, Project};
use dc_ipc::Listener;
use dc_ops::live::{Answer, CHANGED, LOCK_OWNER, Request};
use dc_project::history_log::Event;
use dc_project::{HistoryLog, ProjectLock, Working};

use crate::live::Editor;
use crate::test_util::{Fixture, defaults_file};
use crate::{CliError, Local, Outcome, run_with};

fn channel(case: &str) -> String {
    dc_ipc::socket_name(&format!("test-live-{case}-{}", std::process::id())).unwrap()
}

/// The editor holding `f`'s project until it gets `Show`; then it gives back its state.
struct FakeEditor {
    channel: String,
    server: JoinHandle<(Project, History)>,
}

impl FakeEditor {
    fn start(f: &Fixture, case: &str) -> Self {
        let channel = channel(case);
        let listener = Listener::bind(&channel).unwrap();
        let lock = ProjectLock::acquire(&f.dir, LOCK_OWNER).unwrap();
        let dir = f.dir.clone();
        let server = std::thread::spawn(move || {
            let _lock = lock;
            let working = Working::load(&dir, true).unwrap();
            let (mut project, mut history) = (working.project, working.history);
            let log = HistoryLog::new(&dir);
            let (mut revision, mut pending) = (0, Vec::<Event>::new());
            loop {
                let mut connection = listener.accept().unwrap();
                let answer = match connection.receive().unwrap() {
                    Request::Show => {
                        connection.send(&Answer::Shown).unwrap();
                        return (project, history);
                    }
                    Request::Open { root } => {
                        assert_eq!(root, dir.root());
                        log.append(&pending).unwrap();
                        pending.clear();
                        dir.save_session(&project).unwrap();
                        Answer::Open { revision }
                    }
                    Request::Write {
                        revision: base,
                        events,
                        ..
                    } if base == revision => {
                        pending.extend(events.iter().cloned());
                        dc_ops::live::replay(&mut project, &mut history, events);
                        revision += 1;
                        Answer::Written { revision }
                    }
                    Request::Write { .. } => Answer::Refused(CHANGED.to_owned()),
                    Request::Edit { root } if root == dir.root() => Answer::Opened,
                    Request::Edit { .. } => Answer::Asking,
                    Request::Save { .. } => {
                        log.append(&pending).unwrap();
                        pending.clear();
                        dir.save(&project).unwrap();
                        Answer::Saved
                    }
                };
                connection.send(&answer).unwrap();
            }
        });
        Self { channel, server }
    }

    fn stop(self) -> (Project, History) {
        let shown: Answer = dc_ipc::request(&self.channel, &Request::Show).unwrap();
        assert_eq!(shown, Answer::Shown);
        self.server.join().unwrap()
    }
}

fn run(f: &Fixture, line: &str, channel: &str) -> Outcome {
    let defaults = defaults_file(f._tmp.path());
    let local = Local {
        defaults: Some(&defaults),
        app_channel: Some(channel),
        ..Local::default()
    };
    let args = line.split(' ').map(str::to_owned).collect();
    run_with(args, &f.cwd, &f.registry, local)
}

fn working(f: &Fixture) -> Project {
    Working::load(&f.dir, false).unwrap().project
}

/// Two projects with the same history: the editor holds the first, the second is unlocked.
fn pair(case: &str) -> (Fixture, FakeEditor, Fixture) {
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
