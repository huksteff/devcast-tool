//! The worker protocol and the job over fake worker processes (no Whisper): `cmd /C type` prints
//! a prepared answer, PowerShell stands for a worker that keeps working until it is killed.

use std::process::{Command, Stdio};

use dc_core::{Project, Range, SourceTime};
use dc_fx_captions::{NewCue, Word};
use dc_ui_kit::strings::captions as text;

use super::job::{Job, Spawn};
use super::{Message, Request};
use crate::settings::SpeechModel;

/// A cue of one word said from `start` to `end` seconds of `media/screen.mp4`.
pub(crate) fn cue(said: &str, start: f64, end: f64) -> NewCue {
    let range = Range::new(
        SourceTime::from_secs_f64(start),
        SourceTime::from_secs_f64(end),
    )
    .unwrap();
    NewCue {
        media: "media/screen.mp4".to_owned(),
        range,
        words: vec![Word {
            text: said.to_owned(),
            range,
        }],
    }
}

/// A worker that answers with `messages` and ends; the folder of its answer lives while the
/// returned guard does.
pub(crate) fn answering(messages: &[Message]) -> (tempfile::TempDir, Spawn) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("answer.txt");
    let lines: Vec<String> = messages.iter().map(|m| m.line() + "\n").collect();
    std::fs::write(&file, lines.concat()).unwrap();
    let spawn: Spawn = Box::new(move || {
        Command::new("cmd")
            .args(["/C", "type"])
            .arg(&file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    });
    (temp, spawn)
}

/// A worker that says it is a quarter done and then works for ten minutes.
pub(crate) fn endless() -> Spawn {
    Box::new(|| {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::Out.WriteLine('progress 0.250'); [Console]::Out.Flush(); Start-Sleep -Seconds 600",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    })
}

fn request() -> Request {
    Request {
        dir: "Демо-запись.dcproj".into(),
        project: Project::default(),
        model: SpeechModel::Fast,
        language: None,
        tracks: Vec::new(),
    }
}

fn run(spawn: &Spawn) -> Job {
    Job::start(spawn().unwrap(), &request(), || {}).unwrap()
}

#[test]
fn lines_of_the_messages_read_back() {
    let messages = [
        Message::Progress(0.25),
        Message::Cues(vec![cue("Привет", 1.0, 2.0)]),
        Message::Failed("нет звука".to_owned()),
    ];
    for message in messages {
        assert_eq!(Message::parse(&message.line()), Some(message));
    }
    let broken = Message::Failed("первая\nвторая".to_owned()).line();
    assert_eq!(broken.lines().count(), 1, "one line however long the text");
    assert_eq!(Message::parse("что-то ещё"), None);
}

#[test]
fn the_answer_of_a_worker_comes_in_order() {
    let cues = vec![cue("Привет", 1.0, 2.0), cue("мир", 2.5, 3.0)];
    let (_temp, spawn) = answering(&[Message::Progress(0.5), Message::Cues(cues.clone())]);
    let job = run(&spawn);
    assert_eq!(job.next(), Some(Message::Progress(0.5)));
    assert_eq!(job.next(), Some(Message::Cues(cues)));
    assert_eq!(job.next(), None, "nothing after the last message");
}

#[test]
fn a_failure_and_a_missing_answer_are_messages() {
    let (_temp, spawn) = answering(&[Message::Failed("речи не найдено".to_owned())]);
    let job = run(&spawn);
    assert_eq!(
        job.next(),
        Some(Message::Failed("речи не найдено".to_owned()))
    );
    let (_temp, spawn) = answering(&[Message::Progress(0.1)]);
    let job = run(&spawn);
    assert_eq!(job.next(), Some(Message::Progress(0.1)));
    assert_eq!(
        job.next(),
        Some(Message::Failed(text::NO_ANSWER.to_owned()))
    );
}

#[test]
fn cancel_kills_the_worker() {
    let job = run(&endless());
    assert_eq!(job.next(), Some(Message::Progress(0.25)), "it works");
    let status = job.cancel().unwrap();
    assert!(!status.success(), "killed, not finished: {status}");
}

/// The worker's own path on a copy of the speech fixture: real Whisper, so it needs the fast
/// model in `%APPDATA%\DevCast\models` (the settings download it) —
/// `cargo nextest run -p dc-app --run-ignored only worker_recognizes`.
#[test]
#[ignore = "real Whisper and the fast model on disk"]
fn the_worker_recognizes_the_speech_fixture() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/speech.dcproj");
    let source = dc_project::ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = source
        .save_as(&project, temp.path().join("речь для субтитров.dcproj"))
        .unwrap();
    let request = Request {
        dir: dir.root().to_path_buf(),
        project: project.clone(),
        ..request()
    };
    let cues = super::worker::recognize(&request).unwrap();
    assert!(!cues.is_empty());
    let mut project = project;
    let command = dc_fx_captions::CaptionsGen { cues };
    let mut history = dc_core::History::default();
    let name = dc_fx_captions::CaptionsGen::NAME;
    dc_ops::apply_commands(&mut project, &mut history, name, &[&command], false).unwrap();
}
