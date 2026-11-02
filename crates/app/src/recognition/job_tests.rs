//! The worker protocol and the job over fake worker processes (no Whisper): `cmd /C type` prints
//! a prepared answer, PowerShell stands for a worker that keeps working until it is killed.

use std::process::{Command, Stdio};

use dc_core::{Project, Range, SourceTime};
use dc_fx_captions::{NewCue, Word};
use dc_ui_kit::strings::captions as text;

use super::job::{Job, Spawn, beside};
use super::{Message, Request, Speech};
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
        dir: "Демо-запись.devcast".into(),
        project: Project::default(),
        speech: Speech {
            model: SpeechModel::Fast.model(),
            language: None,
            tracks: Vec::new(),
        },
    }
}

fn run(spawn: &Spawn) -> Job {
    Job::start(spawn().unwrap(), &request(), || {}).unwrap()
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

#[test]
fn the_worker_is_the_cli_beside_the_app() {
    let temp = tempfile::Builder::new()
        .prefix("Программа DevCast ")
        .tempdir()
        .unwrap();
    let app = temp.path().join("devcast-app.exe");
    let missing = beside(&app).unwrap_err();
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    assert!(missing.to_string().contains("devcast.exe"), "{missing}");
    std::fs::write(temp.path().join("devcast.exe"), b"").unwrap();
    assert_eq!(beside(&app).unwrap(), temp.path().join("devcast.exe"));
}
