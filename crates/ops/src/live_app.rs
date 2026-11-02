//! What `devcast` asks of the running app itself rather than of the editor's project (owner's
//! rule 2026-11-02: everything the app does is a command): its recording (`record --app`,
//! `record stop|pause|resume|cancel`), its state (`app status`). The requests are in
//! [`crate::live::Request`]; here are their answers.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::export_jobs::Job;

/// What `devcast record stop|pause|resume|cancel` asks of the app's recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordControl {
    /// As «Стоп»: the project is saved and opens in the editor; a countdown is cancelled.
    Stop,
    Pause,
    Resume,
    /// As «Отменить запись?» answered «Да», without the question: the recording's project is
    /// deleted; a countdown is cancelled.
    Cancel,
}

/// The app's answer about its recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordAnswer {
    /// The first frame is recorded into the project folder `root`.
    Started { root: PathBuf },
    /// Paused; `recorded`: s recorded so far without the pauses.
    Paused { recorded: f64 },
    /// Going on; `recorded`: s recorded so far without the pauses.
    Resumed { recorded: f64 },
    /// Saved in the project folder `root` (with its auto zooms); the editor opens it.
    Stopped { root: PathBuf },
    /// The recording and its project folder `root` are deleted.
    Cancelled { root: PathBuf },
    /// The countdown was cancelled before the recording started; nothing was recorded.
    CountdownCancelled,
    /// The request does not fit the recording's state now; nothing changed.
    NotNow(RecordingState),
    /// The recording did not start or did not finish: the app's message (it shows it too).
    Failed(String),
}

/// Where the app's recording is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RecordingState {
    Idle,
    /// The countdown before the start: `left` s.
    Countdown {
        left: u32,
    },
    /// The devices open; the recording's project folder `root`.
    Starting {
        root: Option<PathBuf>,
    },
    Recording {
        root: PathBuf,
        /// s recorded without the pauses.
        recorded: f64,
        paused: bool,
        /// The app asks «Отменить запись?» (the recording is paused meanwhile).
        asking: bool,
    },
    /// Stopped: the tracks are joined and the project saved.
    Preparing {
        root: PathBuf,
    },
    /// Cancelled: the files are deleted.
    Cancelling {
        root: PathBuf,
    },
}

/// `devcast app status`: one place to poll the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppStatus {
    pub recording: RecordingState,
    /// The project open in the editor, if any.
    pub editor: Option<EditorState>,
    /// The jobs of the app's export queue, ids as the app numbers them.
    pub exports: Vec<Job>,
}

/// The editor's project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorState {
    pub root: PathBuf,
    /// There are changes not in `project.json` yet.
    pub unsaved: bool,
    pub captions: CaptionsState,
}

/// «Создать субтитры» of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CaptionsState {
    Idle,
    /// The speech model is being downloaded: its share 0..1 once the size is known.
    Downloading {
        done: Option<f32>,
    },
    /// The speech is being recognized: its share 0..1 once the worker said.
    Recognizing {
        done: Option<f32>,
    },
}
