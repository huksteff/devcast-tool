//! `devcast` asks the app itself (owner's rule 2026-11-02: everything the app does is a
//! command, so an agent never clicks): `record --app` and `record stop|pause|resume|cancel` go
//! the way of the HUD's buttons (`session.rs`), `export` of the editor's project joins the app's
//! queue, `captions stop` is «Отменить» of the captions panel, `app status` reads it all. A run
//! whose answer is not known yet (the start, the save) waits in [`Waiters`] until the recorder's
//! event; the channel serves other runs meanwhile (`instance.rs`).

use std::path::PathBuf;

use dc_ops::export::Spec;
use dc_ops::export_jobs::{Job, finished};
use dc_ops::live::{
    Answer, AppStatus, CaptionsState, EditorState, RecordAnswer, RecordControl, Request,
};

use crate::app::Shared;
use crate::editor::generation::Phase;
use crate::instance::Reply;
use crate::session::Session;

/// What a waiting run asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wait {
    Start,
    Stop,
    Cancel,
    Pause,
    Resume,
}

/// What happened to the recording, as the waiting runs hear it.
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    /// The first frame, into this project folder.
    Started(PathBuf),
    /// s recorded without the pauses.
    Paused(f64),
    Resumed(f64),
    /// A pause or resume failed.
    Error(String),
    /// Saved in this project folder.
    Stopped(PathBuf),
    /// Deleted with this project folder.
    Cancelled(PathBuf),
    /// Did not start or did not finish.
    Failed(String),
    CountdownCancelled,
}

/// The runs waiting for the recording.
#[derive(Default)]
pub struct Waiters(Vec<(Wait, Reply)>);

impl Waiters {
    fn push(&mut self, wait: Wait, reply: Reply) {
        self.0.push((wait, reply));
    }

    /// Answers every run that waited for `heard`; the rest wait on.
    pub fn hear(&mut self, heard: Heard) {
        for (wait, reply) in std::mem::take(&mut self.0) {
            match answer(wait, &heard) {
                Some(answer) => reply.send(Answer::Record(answer)),
                None => self.0.push((wait, reply)),
            }
        }
    }
}

/// The answer `heard` gives a run that waits for `wait`, if it does.
fn answer(wait: Wait, heard: &Heard) -> Option<RecordAnswer> {
    Some(match (heard, wait) {
        (Heard::Stopped(root), _) => RecordAnswer::Stopped { root: root.clone() },
        (Heard::Cancelled(root), _) => RecordAnswer::Cancelled { root: root.clone() },
        (Heard::Failed(message), _) => RecordAnswer::Failed(message.clone()),
        (Heard::Started(root), Wait::Start) => RecordAnswer::Started { root: root.clone() },
        (Heard::CountdownCancelled, Wait::Start | Wait::Stop | Wait::Cancel) => {
            RecordAnswer::CountdownCancelled
        }
        (Heard::Paused(recorded), Wait::Pause) => RecordAnswer::Paused {
            recorded: *recorded,
        },
        (Heard::Resumed(recorded), Wait::Resume) => RecordAnswer::Resumed {
            recorded: *recorded,
        },
        (Heard::Error(message), Wait::Pause | Wait::Resume) => {
            RecordAnswer::Failed(message.clone())
        }
        _ => return None,
    })
}

impl Shared {
    /// Answers a request of the app channel now, or keeps `reply` until the answer is known.
    pub fn call(&mut self, request: Request, reply: Reply) {
        match request {
            Request::Record => self.record_asked(reply),
            Request::RecordControl(control) => self.control_asked(control, reply),
            Request::Export(spec) => self.export_asked(*spec, reply),
            Request::Exports => reply.send(Answer::Exports {
                jobs: self.export_jobs(),
            }),
            Request::CancelExports { id } => reply.send(Answer::ExportsCancelled {
                ids: self
                    .exports
                    .as_ref()
                    .map(|e| e.cancel(id))
                    .unwrap_or_default(),
            }),
            Request::StopCaptions { root } => reply.send(self.stop_captions(&root)),
            Request::Status => reply.send(Answer::Status(Box::new(self.status()))),
            Request::Update { install } => self.update_asked(install, reply),
            request => reply.send(self.live(request)),
        }
    }

    /// `devcast record --app`: «Запись» of the HUD; the run hears when the first frame is in.
    fn record_asked(&mut self, reply: Reply) {
        if !self.session.is_idle() {
            return reply.send(Answer::Record(RecordAnswer::NotNow(self.session.state())));
        }
        self.record();
        self.waiters.push(Wait::Start, reply);
    }

    /// `devcast record stop|pause|resume|cancel`: as the HUD's buttons and hotkeys, but a cancel
    /// is the answer to «Отменить запись?» itself, so the question is not shown.
    fn control_asked(&mut self, control: RecordControl, reply: Reply) {
        let state = self.session.state();
        let (wait, toggle) = match (control, &self.session) {
            (
                RecordControl::Stop | RecordControl::Cancel,
                Session::Countdown(_) | Session::Launch,
            ) => {
                self.session = Session::Idle;
                self.waiters.hear(Heard::CountdownCancelled);
                return reply.send(Answer::Record(RecordAnswer::CountdownCancelled));
            }
            (RecordControl::Stop, Session::Starting { .. } | Session::Live(_)) => {
                self.stop();
                (Wait::Stop, false)
            }
            (RecordControl::Cancel, Session::Starting { .. }) => {
                self.cancel_starting();
                (Wait::Cancel, false)
            }
            (RecordControl::Cancel, Session::Live(_)) => {
                self.answer_cancel(true);
                (Wait::Cancel, false)
            }
            (RecordControl::Pause, Session::Live(live)) if !live.paused() && !live.asking => {
                (Wait::Pause, true)
            }
            (RecordControl::Resume, Session::Live(live)) if live.paused() && !live.asking => {
                (Wait::Resume, true)
            }
            _ => return reply.send(Answer::Record(RecordAnswer::NotNow(state))),
        };
        if toggle {
            self.toggle_pause();
        }
        self.waiters.push(wait, reply);
    }

    /// `devcast export` of the editor's project: the job joins the app's queue and the run
    /// hears when it has ended.
    fn export_asked(&mut self, spec: Spec, reply: Reply) {
        let Some(exports) = self.exports.clone() else {
            return reply.send(Answer::NoExports);
        };
        let id = exports.push(spec);
        let waiting = std::thread::Builder::new()
            .name("devcast-export-reply".to_owned())
            .spawn(move || {
                let state = exports.wait(id).unwrap_or(dc_encode::JobState::Cancelled);
                reply.send(Answer::Exported(finished(id, state)));
            });
        if let Err(error) = waiting {
            tracing::error!("ответ об экспорте p{id} не будет отправлен: {error}");
        }
    }

    fn export_jobs(&self) -> Vec<Job> {
        let jobs = self
            .exports
            .as_ref()
            .map(|e| e.status())
            .unwrap_or_default();
        jobs.iter().map(Job::of).collect()
    }

    /// `devcast captions stop`: «Отменить» of the captions panel in the editor of `root`.
    fn stop_captions(&mut self, root: &std::path::Path) -> Answer {
        let Some(editor) = self.editor_of(root) else {
            return Answer::NotOpen;
        };
        let stopped = editor.captions.phase() != Phase::Idle;
        editor.captions.cancel();
        editor.detached.wake_editor = true;
        self.ctx.request_repaint_of(eframe::egui::ViewportId::ROOT);
        Answer::CaptionsStopped { stopped }
    }

    /// `devcast app status`.
    pub fn status(&self) -> AppStatus {
        let editor = self.editor.as_ref().map(|e| EditorState {
            root: e.doc.dir().root().to_path_buf(),
            unsaved: e.doc.unsaved(),
            captions: match e.captions.phase() {
                Phase::Idle => CaptionsState::Idle,
                Phase::Downloading(done) => CaptionsState::Downloading { done },
                Phase::Recognizing(done) => CaptionsState::Recognizing { done },
            },
        });
        AppStatus {
            recording: self.session.state(),
            editor,
            exports: self.export_jobs(),
        }
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
