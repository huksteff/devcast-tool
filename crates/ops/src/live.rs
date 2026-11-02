//! The app channel `devcast-app-<user>` (dc-ipc, decision 011): a second start of the app asks
//! the running one to show itself, and `devcast` edits the project the editor holds (7.32).
//!
//! A run of `devcast` on that project asks [`Request::Open`]: the editor writes what is not on
//! disk yet, so the run reads the editor's exact working state from the files and does its
//! command the way it does without the editor (same code, same answers and errors). What it did
//! goes back as history events in [`Request::Write`]; the editor takes them only when its state
//! is still the one the run read (`revision`), so they land on the same state and the result is
//! the project the run computed, one step of the editor's history.

use std::path::PathBuf;

use dc_core::{History, Project};
use dc_project::history_log::Event;
use serde::{Deserialize, Serialize};

/// Service name of the channel (`dc_ipc::socket_name`).
pub const SERVICE: &str = "app";

/// Owner name in `lock-owner.json` while the app holds a project: `devcast` then edits it
/// through the channel.
pub const LOCK_OWNER: &str = "devcast-app";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    /// A second start of the app: the running one shows its window.
    Show,
    /// A `devcast` run opens the project in folder `root`: when the editor has it, its working
    /// state goes to disk (`session.json`, `history.jsonl`) and the answer is [`Answer::Open`].
    Open { root: PathBuf },
    /// The history events a run made of the working state at `revision` (`Answer::Open`).
    Write {
        root: PathBuf,
        revision: u64,
        events: Vec<Event>,
    },
    /// `devcast save`: the working state becomes `project.json` (Ctrl+S).
    Save { root: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Shown,
    /// The editor has the project; its working state on disk is this revision.
    Open {
        revision: u64,
    },
    /// The editor does not have the project in this folder open.
    NotOpen,
    /// The events are taken: one more step of the editor's history; the new revision.
    Written {
        revision: u64,
    },
    Saved,
    /// Not done; the text says why and what to do.
    Refused(String),
}

/// Refusal of a write over a state that changed after the run read it.
pub const CHANGED: &str = "проект изменился в редакторе, пока шла команда: запусти её ещё раз";

/// Takes `events` another writer made of exactly this `project` and `history`.
pub fn replay(project: &mut Project, history: &mut History, events: Vec<Event>) {
    for event in events {
        match event {
            Event::Do(change) => history.push(project, change),
            Event::Undo => {
                history.undo(project);
            }
            Event::Redo => {
                history.redo(project);
            }
        }
    }
}
