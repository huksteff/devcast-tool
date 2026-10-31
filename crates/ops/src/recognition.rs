//! The protocol of the recognition process (docs/decisions/018-recognition-process.md): the
//! editor does not link Whisper, so it starts `devcast` with [`WORKER_ARG`] beside its own exe
//! and cancels by killing it. The request is one JSON [`Request`] on the worker's stdin; the
//! answer is lines of [`Message`] on its stdout: `progress <0..1>` while it works, then
//! `cues <JSON>` or `error <text>`. Both sides link this module, so they speak the same lines.

use std::path::PathBuf;

use dc_core::Project;
use dc_fx_captions::NewCue;
use dc_models::Model;
use serde::{Deserialize, Serialize};

/// The hidden argument that turns `devcast` into the recognition worker.
pub const WORKER_ARG: &str = "--captions-worker";

/// What to recognize: the model, the language (`None` — detected) and the tracks, the first
/// one wins where speech overlaps (none — `mic`, a recording without it — its first track).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speech {
    pub model: Model,
    pub language: Option<String>,
    pub tracks: Vec<String>,
}

/// One recognition: the project as the editor has it now (unsaved changes too).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The project folder: recordings and the words cache.
    pub dir: PathBuf,
    pub project: Project,
    pub speech: Speech,
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

#[cfg(test)]
mod tests {
    use dc_core::{Range, SourceTime};
    use dc_fx_captions::Word;

    use super::*;

    fn cue(said: &str) -> NewCue {
        let range = Range::new(
            SourceTime::from_secs_f64(1.0),
            SourceTime::from_secs_f64(2.0),
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

    #[test]
    fn lines_of_the_messages_read_back() {
        let messages = [
            Message::Progress(0.25),
            Message::Cues(vec![cue("Привет")]),
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
    fn a_request_reads_back() {
        let request = Request {
            dir: "Демо запись.dcproj".into(),
            project: Project::default(),
            speech: Speech {
                model: Model::LargeV3Turbo,
                language: Some("ru".to_owned()),
                tracks: vec!["mic".to_owned(), "system".to_owned()],
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&json).unwrap(), request);
    }
}
