//! `devcast --captions-worker`, the recognition process of the editor, through the real exe:
//! the request on stdin, the answer lines on stdout (`dc_ops::recognition`).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use dc_core::Project;
use dc_models::Model;
use dc_ops::recognition::{Message, Request, Speech, WORKER_ARG};

/// The answer of the worker to `input`: its messages, in order.
#[cfg(test)]
fn answer(input: &str) -> Vec<Message> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_devcast"))
        .arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out.status);
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|line| Message::parse(line).unwrap_or_else(|| panic!("not a message: {line}")))
        .collect()
}

#[cfg(test)]
fn request(dir: &Path, project: Project) -> String {
    let request = Request {
        dir: dir.to_path_buf(),
        project,
        speech: Speech {
            model: Model::Small,
            language: None,
            tracks: Vec::new(),
        },
    };
    serde_json::to_string(&request).unwrap()
}

#[test]
fn a_broken_request_is_an_error_line() {
    let messages = answer("{не json");
    assert!(
        matches!(messages.as_slice(), [Message::Failed(text)] if text.contains("испорчен")),
        "{messages:?}"
    );
}

#[test]
fn a_project_without_recordings_has_no_speech() {
    let temp = tempfile::tempdir().unwrap();
    let dir = dc_project::ProjectDir::create(temp.path().join("пустой проект.dcproj")).unwrap();
    let messages = answer(&request(dir.root(), Project::default()));
    assert!(
        matches!(messages.as_slice(), [Message::Failed(text)] if text.contains("речи не найдено")),
        "{messages:?}"
    );
}

/// Real Whisper on a copy of the speech fixture: it needs the fast model in
/// `%APPDATA%\DevCast\models` — `cargo nextest run -p dc-cli --run-ignored only worker`.
#[test]
#[ignore = "real Whisper and the fast model on disk"]
fn the_worker_recognizes_the_speech_fixture() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/speech.dcproj");
    let source = dc_project::ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = source
        .save_as(&project, temp.path().join("речь для субтитров.dcproj"))
        .unwrap();
    let messages = answer(&request(dir.root(), project.clone()));
    let Some(Message::Cues(cues)) = messages.last() else {
        panic!("no cues: {messages:?}");
    };
    assert!(!cues.is_empty());
    assert!(
        messages[..messages.len() - 1]
            .iter()
            .all(|m| matches!(m, Message::Progress(_))),
        "{messages:?}"
    );
    let mut project = project;
    let command = dc_fx_captions::CaptionsGen { cues: cues.clone() };
    let mut history = dc_core::History::default();
    let name = dc_fx_captions::CaptionsGen::NAME;
    dc_ops::apply_commands(&mut project, &mut history, name, &[&command], false).unwrap();
}
