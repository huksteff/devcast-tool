//! `devcast` on the editor's project (7.32): a run reads the state the editor put on disk,
//! applies its command there as `devcast` does and sends the events; the editor's project is
//! then the run's, and undo in the editor reverts it.

use std::time::Instant;

use dc_core::Change;
use dc_ops::Step;
use dc_ops::live::{Answer, CHANGED, Request};
use dc_project::Working;
use dc_project::history_log::Event;

use super::editing_tests::{by_cli, json, undone};
use super::tests::{editor, project};
use super::*;

/// What a `devcast <op> <args>` run does after `Open`: the change it makes of the state on disk.
fn cli_change(editor: &EditorWindow, op: &str, args: &[&str]) -> Change {
    let registry = dc_ops::Registry::devcast().unwrap();
    let mut working = Working::load(editor.doc.dir(), false).unwrap();
    let info = registry.find(op).unwrap();
    let words: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let steps = [Step::new(op, dc_ops::parse_args(info, &words).unwrap())];
    let applied = dc_ops::apply(
        &registry,
        &mut working.project,
        &mut working.history,
        &steps,
        false,
    )
    .unwrap();
    assert!(applied.changed);
    working.history.changes().last().unwrap().clone()
}

fn open(editor: &mut EditorWindow) -> u64 {
    let root = editor.doc.dir().root().to_path_buf();
    match editor.live(Request::Open { root }, Instant::now()).0 {
        Answer::Open { revision } => revision,
        other => panic!("{other:?}"),
    }
}

fn write(editor: &mut EditorWindow, revision: u64, events: Vec<Event>) -> Answer {
    let root = editor.doc.dir().root().to_path_buf();
    let request = Request::Write {
        root,
        revision,
        events,
    };
    editor.live(request, Instant::now()).0
}

#[test]
fn a_cli_command_is_a_step_of_the_editor_history() {
    let (_temp, mut editor) = editor();
    let revision = open(&mut editor);
    let change = cli_change(&editor, "keys", &["--show", "true"]);
    let written = write(&mut editor, revision, vec![Event::Do(change)]);
    assert_eq!(written, Answer::Written { revision: 1 });
    assert_eq!(
        json(editor.doc.project()),
        json(&by_cli("keys --show true"))
    );
    assert!(editor.doc.unsaved());
    editor.doc.travel(Way::Undo, Instant::now()).unwrap();
    assert_eq!(
        json(editor.doc.project()),
        undone(&project(), editor.doc.project())
    );
}

#[test]
fn the_run_reads_what_the_editor_has_not_written_yet() {
    let (_temp, mut editor) = editor();
    let now = Instant::now();
    editor.doc.apply(&[Step::new("keys", show())], now).unwrap();
    let revision = open(&mut editor);
    assert_eq!(revision, 1);
    let on_disk = Working::load(editor.doc.dir(), false).unwrap();
    assert_eq!(json(&on_disk.project), json(editor.doc.project()));
    assert_eq!(on_disk.history.changes().len(), 1);
}

#[test]
fn a_write_over_a_changed_state_is_refused() {
    let (_temp, mut editor) = editor();
    let revision = open(&mut editor);
    let change = cli_change(&editor, "keys", &["--show", "true"]);
    editor
        .doc
        .apply(&[Step::new("captions style", hide())], Instant::now())
        .unwrap();
    let before = json(editor.doc.project());
    let refused = write(&mut editor, revision, vec![Event::Do(change)]);
    assert_eq!(refused, Answer::Refused(CHANGED.to_owned()));
    assert_eq!(json(editor.doc.project()), before);
}

#[test]
fn another_project_is_not_open() {
    let (_temp, mut editor) = editor();
    let other = tempfile::tempdir().unwrap();
    let request = Request::Open {
        root: other.path().to_path_buf(),
    };
    let answer = editor.live(request, Instant::now()).0;
    assert_eq!(answer, Answer::NotOpen);
}

#[test]
fn requests_come_over_the_app_channel() {
    let (_temp, mut editor) = editor();
    let name = format!("devcast-test-live-{}", std::process::id());
    let listener = dc_ipc::Listener::bind(&name).unwrap();
    let calls = crate::instance::serve(listener, egui::Context::default());
    let root = editor.doc.dir().root().to_path_buf();
    let cli = std::thread::spawn(move || {
        dc_ipc::request::<_, Answer>(&name, &Request::Open { root }).unwrap()
    });
    let call = calls.recv().unwrap();
    let (answer, _) = editor.live(call.request, Instant::now());
    call.reply.send(answer).unwrap();
    assert_eq!(cli.join().unwrap(), Answer::Open { revision: 0 });
}

fn show() -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([("show".to_owned(), serde_json::Value::Bool(true))])
}

fn hide() -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([("show".to_owned(), serde_json::Value::Bool(false))])
}
