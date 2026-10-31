//! The recognition worker: the app started with `--captions-worker`. It reads the [`Request`]
//! from stdin, recognizes as `devcast captions gen` does and writes [`Message`] lines to
//! stdout. Nobody waits for it but the editor that started it: once the editor is gone (its end
//! of the pipe closed), the worker stops.

use std::cell::Cell;
use std::io::{Read, Write};
use std::rc::Rc;

use dc_ops::captions::{OnProgress, Speech};
use dc_project::ProjectDir;

use super::{Message, Request};

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

pub(super) fn recognize(request: &Request) -> Result<Vec<dc_fx_captions::NewCue>, String> {
    let dir = ProjectDir::open(&request.dir).map_err(|e| e.to_string())?;
    let speech = Speech {
        model: request.model.model(),
        language: request.language.clone(),
        tracks: request.tracks.clone(),
    };
    let last = Cell::new(None);
    let progress: OnProgress = Rc::new(move |done: f32| {
        let percent = (done.clamp(0.0, 1.0) * 100.0) as u32;
        if last.replace(Some(percent)) != Some(percent) {
            say(&Message::Progress(percent as f32 / 100.0));
        }
    });
    dc_ops::captions::generate(&dir, &request.project, &speech, &progress)
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
