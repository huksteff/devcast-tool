//! Recording state changes reported by a [`crate::Recorder`], stamped on the host clock.

use serde::{Deserialize, Serialize};

/// Time on the monotonic system clock shared by video, audio and input (QPC on Windows),
/// in microseconds since an arbitrary start (boot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostTime(i64);

impl HostTime {
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    pub const fn micros(self) -> i64 {
        self.0
    }
}

/// What happened to a recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RecordingEvent {
    Started {
        at: HostTime,
    },
    Paused {
        at: HostTime,
    },
    Resumed {
        at: HostTime,
    },
    /// Finished normally; the files are complete.
    Stopped {
        at: HostTime,
    },
    /// The recorded window was closed; the recording stops as after [`Self::Stopped`].
    SourceClosed {
        at: HostTime,
    },
    /// The recording broke off; `message` says what happened and what to do.
    Failed {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_tagged_by_event() {
        let events = [
            (
                RecordingEvent::Started {
                    at: HostTime::from_micros(1_500_000),
                },
                r#"{"event":"started","at":1500000}"#,
            ),
            (
                RecordingEvent::SourceClosed {
                    at: HostTime::from_micros(7),
                },
                r#"{"event":"source_closed","at":7}"#,
            ),
            (
                RecordingEvent::Failed {
                    message: "диск переполнен".to_owned(),
                },
                r#"{"event":"failed","message":"диск переполнен"}"#,
            ),
        ];
        for (event, json) in events {
            assert_eq!(serde_json::to_string(&event).unwrap(), json);
            assert_eq!(serde_json::from_str::<RecordingEvent>(json).unwrap(), event);
        }
    }
}
