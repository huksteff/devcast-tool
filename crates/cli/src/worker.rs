//! The recognition worker: `devcast --captions-worker`, a hidden mode the editor starts
//! (docs/decisions/018-recognition-process.md). It reads the [`Request`] from stdin, recognizes
//! as `devcast captions gen` does (`speech::generate`) and writes [`Message`] lines to stdout.
//! Nobody waits for it but the editor that started it: once the editor is gone (its end of the
//! pipe closed), the worker stops.

use std::cell::Cell;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::rc::Rc;

use dc_fx_captions::NewCue;
use dc_ops::recognition::{Message, Request, WORKER_ARG};
use dc_project::ProjectDir;

use crate::speech::OnProgress;

/// The arguments (without the program) ask for the worker: exactly [`WORKER_ARG`].
pub fn is_worker(args: &[OsString]) -> bool {
    matches!(args, [flag] if flag == WORKER_ARG)
}

/// Serves one request: every outcome, a panic too, is an answer line.
pub fn serve() {
    std::panic::set_hook(Box::new(|info| {
        say(&Message::Failed(format!("распознавание упало: {info}")));
    }));
    let answer = match request().and_then(|r| recognize(&r)) {
        Ok(cues) => Message::Cues(cues),
        Err(error) => Message::Failed(error),
    };
    say(&answer);
}

fn request() -> Result<Request, String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("запрос распознавания не прочитан: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("запрос распознавания испорчен: {e}"))
}

pub(crate) fn recognize(request: &Request) -> Result<Vec<NewCue>, String> {
    let dir = ProjectDir::open(&request.dir).map_err(|e| e.to_string())?;
    let last = Cell::new(None);
    let progress: OnProgress = Rc::new(move |done: f32| {
        let percent = (done.clamp(0.0, 1.0) * 100.0) as u32;
        if last.replace(Some(percent)) != Some(percent) {
            say(&Message::Progress(percent as f32 / 100.0));
        }
    });
    crate::speech::generate(&dir, &request.project, &request.speech, &progress)
        .map(|command| command.cues)
        .map_err(|e| e.to_string())
}

/// Writes `message` for the editor. A write fails only when the editor closed its end: there
/// is nobody to answer, so the worker ends.
fn say(message: &Message) {
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{}", message.line()).and_then(|()| out.flush());
    if written.is_err() {
        std::process::exit(1);
    }
}
