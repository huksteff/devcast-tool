//! The recorder's events and what each does to the session; the state of the recording as
//! `devcast app status` reports it; the end of a recording when the app quits. Each event is
//! also heard by the `devcast` runs waiting for it (`remote::Waiters`).

use std::path::PathBuf;

use dc_capture::Pauses;
use dc_ops::live::RecordingState;
use dc_ui_kit::strings::hud;

use super::{Live, Session};
use crate::app::Shared;
use crate::hud::popover;
use crate::recorder::{Event, Request};
use crate::remote::Heard;

impl Session {
    /// Stops a running recording and waits for its project; for quitting the app. The folder
    /// of the project it saved, if it did.
    pub fn finish(self) -> Option<PathBuf> {
        let recorder = match self {
            Self::Starting { recorder, .. } | Self::Live(Live { recorder, .. }) => {
                recorder.send(Request::Stop);
                recorder
            }
            Self::Preparing(recorder) | Self::Cancelling(recorder) => recorder,
            Self::Idle | Self::Countdown(_) | Self::Launch => return None,
        };
        recorder
            .join_events()
            .into_iter()
            .find_map(|event| match event {
                Event::Stopped(project, _) => Some(project.dir.root().to_path_buf()),
                _ => None,
            })
    }

    /// The state for `devcast app status` and for refusals of `devcast record …`.
    pub fn state(&self) -> RecordingState {
        match self {
            Self::Idle => RecordingState::Idle,
            Self::Countdown(countdown) => RecordingState::Countdown {
                left: countdown.left,
            },
            Self::Launch => RecordingState::Starting { root: None },
            Self::Starting { recorder, .. } => RecordingState::Starting {
                root: Some(recorder.root().to_path_buf()),
            },
            Self::Live(live) => RecordingState::Recording {
                root: live.recorder.root().to_path_buf(),
                recorded: dc_capture::win::now()
                    .map(|now| seconds(live.recorded(now)))
                    .unwrap_or(0.0),
                paused: live.paused(),
                asking: live.asking,
            },
            Self::Preparing(recorder) => RecordingState::Preparing {
                root: recorder.root().to_path_buf(),
            },
            Self::Cancelling(recorder) => RecordingState::Cancelling {
                root: recorder.root().to_path_buf(),
            },
        }
    }
}

/// Microseconds as seconds.
fn seconds(micros: i64) -> f64 {
    micros as f64 / 1_000_000.0
}

impl Shared {
    pub(super) fn on_event(&mut self, event: Event) {
        match (event, std::mem::take(&mut self.session)) {
            (Event::Started(start), Session::Starting { recorder, stop }) => {
                let root = recorder.root().to_path_buf();
                self.session = Session::Live(Live {
                    recorder,
                    start,
                    pauses: Pauses::default(),
                    asking: false,
                });
                self.waiters.hear(Heard::Started(root));
                if stop {
                    self.stop();
                }
            }
            // `devcast record cancel` while it was starting: the cancel is on its way.
            (Event::Started(_), session @ Session::Cancelling(_)) => self.session = session,
            (Event::Paused(at), Session::Live(mut live)) => {
                self.report(live.pauses.pause(at));
                self.waiters.hear(Heard::Paused(seconds(live.recorded(at))));
                self.session = Session::Live(live);
            }
            (Event::Resumed(at), Session::Live(mut live)) => {
                self.report(live.pauses.resume(at));
                self.waiters
                    .hear(Heard::Resumed(seconds(live.recorded(at))));
                self.session = Session::Live(live);
            }
            (Event::Error(message), session) => {
                self.session = session;
                self.waiters.hear(Heard::Error(message.clone()));
                self.report(Err(message));
            }
            (Event::Stopped(project, zooms), session) => {
                finished(session);
                self.waiters
                    .hear(Heard::Stopped(project.dir.root().to_path_buf()));
                self.recorded(*project, zooms);
            }
            (Event::Cancelled, session) => {
                if let Some(root) = root(&session) {
                    self.waiters.hear(Heard::Cancelled(root));
                }
                finished(session);
            }
            (Event::Failed(message), Session::Preparing(recorder)) => {
                let root = recorder.root().to_path_buf();
                recorder.join();
                self.waiters.hear(Heard::Failed(message.clone()));
                self.show_hud();
                self.failure = Some((message, root));
                self.popover = Some(popover::Open::new(popover::Kind::Failed, 0.0));
            }
            (Event::Failed(message), session) => {
                finished(session);
                self.waiters.hear(Heard::Failed(message.clone()));
                self.show_hud();
                self.report(Err(message));
            }
            (event, session) => {
                tracing::warn!("событие записи {event:?} пришло не вовремя");
                self.session = session;
            }
        }
    }

    /// The recording is saved in its project: the editor opens with it, the HUD hides until
    /// the editor closes (v1). What went wrong on the way (`zooms`: the auto zooms) is reported.
    fn recorded(&mut self, project: dc_capture::win::RecordedProject, zooms: Option<String>) {
        tracing::info!("запись сохранена: {}", project.dir.root().display());
        self.open_editor(&project);
        for problem in project.summary.problems() {
            self.report(Err(problem));
        }
        if let Some(problem) = zooms {
            self.report(Err(hud::auto_zooms_failed(&problem)));
        }
    }

    /// `devcast record cancel` of a recording that is starting: the recorder cancels right
    /// after its start (its requests wait meanwhile), without the question.
    pub(crate) fn cancel_starting(&mut self) {
        match std::mem::take(&mut self.session) {
            Session::Starting { recorder, .. } => {
                recorder.send(Request::Cancel);
                self.session = Session::Cancelling(recorder);
            }
            other => self.session = other,
        }
    }
}

/// The project folder of the session's recorder.
fn root(session: &Session) -> Option<PathBuf> {
    match session {
        Session::Starting { recorder, .. }
        | Session::Live(Live { recorder, .. })
        | Session::Preparing(recorder)
        | Session::Cancelling(recorder) => Some(recorder.root().to_path_buf()),
        Session::Idle | Session::Countdown(_) | Session::Launch => None,
    }
}

/// Joins the thread of a recorder that has sent its last event.
fn finished(session: Session) {
    match session {
        Session::Starting { recorder, .. }
        | Session::Live(Live { recorder, .. })
        | Session::Preparing(recorder)
        | Session::Cancelling(recorder) => recorder.join(),
        Session::Idle | Session::Countdown(_) | Session::Launch => {}
    }
}
