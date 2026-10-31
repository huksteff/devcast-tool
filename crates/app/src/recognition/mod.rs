//! Speech recognition in a process of its own (owner's decision 2026-10-29, «вариант 2»):
//! whisper.cpp cannot be stopped inside a process (`dc_transcribe::recognize` has no
//! cancellation), so the editor starts a copy of the app with [`WORKER_ARG`] and cancels by
//! killing it. The copy runs the path of `devcast captions gen` (`dc_ops::captions::generate`)
//! and answers in lines on its stdout; the editor applies the cues as `CaptionsGen`, one undo
//! step, like the CLI.
//!
//! The request is one JSON [`Request`] on the worker's stdin; the answer is lines of
//! [`Message`]: `progress <0..1>` while it works, then `cues <JSON>` or `error <text>`.

pub mod job;
pub mod worker;

#[cfg(test)]
pub(crate) mod job_tests;

use std::path::PathBuf;

use dc_core::Project;
use dc_fx_captions::NewCue;
use serde::{Deserialize, Serialize};

use crate::settings::SpeechModel;

/// The argument that turns the program into the recognition worker.
pub const WORKER_ARG: &str = "--captions-worker";

/// What to recognize: the project as the editor has it now (unsaved changes too) and the
/// choices of the captions panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The project folder: recordings and the words cache.
    pub dir: PathBuf,
    pub project: Project,
    pub model: SpeechModel,
    /// `ru`, `en` …; `None` — detected.
    pub language: Option<String>,
    /// Tracks to listen to, the first wins where speech overlaps; empty — `mic`.
    pub tracks: Vec<String>,
}

/// One line of the worker's answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Share of the work done, 0..1.
    Progress(f32),
    /// The cues found: they replace all cues.
    Cues(Vec<NewCue>),
    /// Why it failed, for the user.
    Failed(String),
}

const PROGRESS: &str = "progress";
const CUES: &str = "cues";
const ERROR: &str = "error";

impl Message {
    /// The line of the message, without the line break; one line however long the text.
    pub fn line(&self) -> String {
        match self {
            Self::Progress(done) => format!("{PROGRESS} {done:.3}"),
            Self::Cues(cues) => match serde_json::to_string(cues) {
                Ok(json) => format!("{CUES} {json}"),
                Err(error) => format!("{ERROR} ответ не собран: {error}"),
            },
            Self::Failed(text) => format!("{ERROR} {}", text.replace(['\r', '\n'], " ")),
        }
    }

    /// The message of a line, `None` for a line that is none.
    pub fn parse(line: &str) -> Option<Self> {
        let (word, rest) = line.trim_end().split_once(' ')?;
        match word {
            PROGRESS => rest.parse().ok().map(Self::Progress),
            CUES => serde_json::from_str(rest).ok().map(Self::Cues),
            ERROR => Some(Self::Failed(rest.to_owned())),
            _ => None,
        }
    }

    /// The last message of an answer: nothing comes after it.
    pub fn is_last(&self) -> bool {
        !matches!(self, Self::Progress(_))
    }
}
