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

use crate::export::Spec;
use crate::export_jobs::{Finished, Job};
pub use crate::live_app::{
    AppStatus, CaptionsState, EditorState, RecordAnswer, RecordControl, RecordingState, UpdateState,
};

/// Service name of the channel (`dc_ipc::socket_name`).
pub const SERVICE: &str = "app";

/// Owner name in `lock-owner.json` while the app holds a project: `devcast` then edits it
/// through the channel.
pub const LOCK_OWNER: &str = "devcast-app";

/// What `devcast record --app --source … --duration …` asks for one recording; the settings of
/// the app stay as they are.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Once {
    /// What to record instead of the source of the settings.
    pub source: Option<dc_capture::Source>,
    /// The recording stops by itself once this many seconds are recorded (pauses do not count).
    pub duration: Option<f64>,
}

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
    /// `devcast open`: the editor opens the project in folder `root`; an editor with unsaved
    /// changes of another project asks about them first.
    Edit { root: PathBuf },
    /// `devcast discard`: the editor drops its working state and history, back to
    /// `project.json` (`Document::discard`).
    Discard { root: PathBuf },
    /// `devcast close`: the editor window closes and the HUD comes back. Unsaved changes are
    /// saved or dropped as `changes` says; without it they refuse the close ([`Answer::Unsaved`]).
    Close {
        root: PathBuf,
        changes: Option<Changes>,
    },
    /// `devcast save --as`: the run has copied the working state of `revision` (it read it after
    /// `Open`) to the project `copy`; the editor does what it does after «Сохранить как…»: drops
    /// its working state and opens the copy. Refused when the state changed since.
    Copied {
        root: PathBuf,
        revision: u64,
        copy: PathBuf,
    },
    /// `devcast font add` put a font into `assets/fonts/` of the project in `root`: no change of
    /// the project, so the editor reads the families of its font pickers again
    /// ([`Answer::FontsRead`]).
    Fonts { root: PathBuf },
    /// `devcast rename`: the editor renames the project folder to `name` (the header's rename).
    Rename { root: PathBuf, name: String },
    /// `devcast config` and `devcast hotkeys`: the app applies the edit to its settings store,
    /// so its windows and global hotkeys follow at once ([`Answer::Configured`]).
    Settings { edit: dc_settings::Edit },
    /// `devcast ls projects`: which project the editor holds ([`Answer::Current`]).
    Current,
    /// `devcast record --app`: as «Запись» of the HUD, with the app's settings (countdown
    /// included) and what [`Once`] changes for this recording. The answer comes once the first
    /// frame is recorded ([`RecordAnswer::Started`]) — with a duration, once the recording has
    /// stopped by itself and is saved ([`RecordAnswer::Stopped`]) — or why not.
    Record(Once),
    /// `devcast record stop|pause|resume|cancel` while no `devcast record` runs. The answer
    /// comes once the recording has done it (stop: once the project is saved).
    RecordControl(RecordControl),
    /// `devcast export` of the project the editor holds: the job joins the app's queue, the
    /// answer ([`Answer::Exported`]) comes when it has ended.
    Export(Box<Spec>),
    /// `devcast export status`: the app's queue ([`Answer::Exports`]).
    Exports,
    /// `devcast export cancel [pN]`: one job of the app's queue or all ([`Answer::ExportsCancelled`]).
    CancelExports { id: Option<u32> },
    /// `devcast captions stop`: the editor of `root` stops «Создать субтитры».
    StopCaptions { root: PathBuf },
    /// `devcast app status` ([`Answer::Status`]).
    Status,
    /// `devcast app update`: as «Проверить обновления» of the settings, the answer
    /// ([`Answer::Update`]) once the check ended; with `install`, a newer version is also
    /// downloaded and the app restarts into it, as «Перезапустить» (not while recording or
    /// exporting).
    Update { install: bool },
    /// `devcast app quit`: as «Выход» of the tray without its question: a recording is stopped
    /// and saved, the editor writes its working state. The answer comes just before the app
    /// ends ([`Answer::Quit`]).
    Quit,
}

/// What `devcast close` does with unsaved changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Changes {
    Save,
    Discard,
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
    /// The editor has the project of `Request::Edit` open.
    Opened,
    /// The editor asks in its window whether to save the changes of the project it had; the
    /// project of `Request::Edit` opens after the answer.
    Asking,
    /// The working state is dropped; `changed`: there were unsaved changes.
    Discarded {
        changed: bool,
    },
    /// The editor window is closed.
    Closed,
    /// `Request::Close` without `changes` while there are unsaved changes: nothing closed.
    Unsaved,
    /// `Request::Close` of a project the editor does not hold: it holds `open`, or nothing.
    Elsewhere {
        open: Option<PathBuf>,
    },
    /// The editor's font pickers list the families of the project's fonts on disk.
    FontsRead,
    /// The project is renamed and open again from this folder.
    Renamed {
        root: PathBuf,
    },
    /// `Request::Settings` is applied and written: the settings after it and what changed.
    Configured(Box<dc_settings::Applied>),
    /// `Request::Settings` is refused; nothing changed.
    NotConfigured(dc_settings::EditError),
    /// The project open in the editor, if any.
    Current {
        root: Option<PathBuf>,
    },
    /// Not done; the text says why and what to do.
    Refused(String),
    /// About the app's recording.
    Record(RecordAnswer),
    /// How the export of `Request::Export` ended (`id` — of the app's queue).
    Exported(Finished),
    /// The app has no export queue (no device for the preview): the run exports by itself.
    NoExports,
    /// The jobs of the app's queue.
    Exports {
        jobs: Vec<Job>,
    },
    /// The jobs of the app's queue that were cancelled.
    ExportsCancelled {
        ids: Vec<u32>,
    },
    /// `Request::StopCaptions`: `stopped` — the recognition (or the wait for its model) was
    /// going on and is stopped.
    CaptionsStopped {
        stopped: bool,
    },
    Status(Box<AppStatus>),
    /// The updates after `Request::Update`.
    Update(UpdateState),
    /// The app ends now; `recorded`: the project of the recording it stopped and saved.
    Quit {
        recorded: Option<PathBuf>,
    },
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
