//! The recording as the HUD sees it: rest → countdown → start → recording (pauses, the cancel
//! question) → preparing → rest. The recorder works on its own thread ([`crate::recorder`]);
//! here are the states, their timing and what each action of the HUD, the tray and the hotkeys
//! does to them. The timer runs on the recorder's clock: host time minus the pauses.

use std::time::{Duration, Instant};

use dc_capture::{HostTime, Pauses};
use eframe::egui;

use crate::app::Shared;
use crate::error::AppError;
use crate::hotkeys::Command;
use crate::hud::popover;
use crate::recorder::{Event, Recorder, Request};
use crate::tray::TrayView;
use dc_ui_kit::strings::tray;

/// One step of the countdown.
const SECOND: Duration = Duration::from_secs(1);

#[derive(Default)]
pub enum Session {
    #[default]
    Idle,
    Countdown(Countdown),
    /// Other windows close this frame; the recorder starts on the next one, so none of them
    /// gets into the first frames.
    Launch,
    /// The recorder opens the devices and waits for the first frame.
    Starting(Recorder),
    Live(Live),
    /// Stop was sent: the tracks are joined and the project saved.
    Preparing(Recorder),
    /// Cancel was sent: the files are deleted.
    Cancelling(Recorder),
}

/// Seconds left before the recording starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Countdown {
    pub left: u32,
    /// When `left` goes down by one.
    pub next: Instant,
    /// Its window has been excluded from capture. Sent once: a command makes the window draw
    /// again, and sending it every frame drew the countdown nonstop and starved the HUD.
    pub excluded: bool,
}

impl Countdown {
    pub fn new(seconds: u32, now: Instant) -> Self {
        Self {
            left: seconds,
            next: now + SECOND,
            excluded: false,
        }
    }

    /// Counts the seconds passed by `now`; true once it reached zero.
    pub fn tick(&mut self, now: Instant) -> bool {
        while self.left > 0 && now >= self.next {
            self.left -= 1;
            self.next += SECOND;
        }
        self.left == 0
    }
}

pub struct Live {
    recorder: Recorder,
    /// Host time of the first frame.
    start: HostTime,
    pauses: Pauses,
    /// «Отменить запись?» is shown (the recording is paused meanwhile).
    pub asking: bool,
}

impl Live {
    pub fn paused(&self) -> bool {
        self.pauses.is_paused()
    }

    /// Recorded by `now`, microseconds, without the pauses.
    pub fn recorded(&self, now: HostTime) -> i64 {
        self.pauses.place(now).time.micros() - self.start.micros()
    }

    /// The timer now; «--:--» (and the reason in the log) if the system clock can't be read.
    pub fn timer(&self) -> String {
        match dc_capture::win::now() {
            Ok(now) => clock(self.recorded(now)),
            Err(error) => {
                tracing::error!("таймер записи: {error}");
                "--:--".to_owned()
            }
        }
    }
}

/// The timer: `MM:SS`, from an hour on `H:MM:SS`.
pub fn clock(micros: i64) -> String {
    let seconds = micros.max(0) / 1_000_000;
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

impl Session {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Stops a running recording and waits for its project; for quitting the app.
    pub fn finish(self) {
        match self {
            Self::Starting(recorder) | Self::Live(Live { recorder, .. }) => {
                recorder.send(Request::Stop);
                recorder.join();
            }
            Self::Preparing(recorder) | Self::Cancelling(recorder) => recorder.join(),
            Self::Idle | Self::Countdown(_) | Self::Launch => {}
        }
    }
}

impl Shared {
    /// «Запись»: closes the other windows (only the HUD is excluded from capture) and starts
    /// the countdown, or the recording at once when the countdown is off.
    pub fn record(&mut self) {
        if !self.session.is_idle() {
            return;
        }
        self.popover = None;
        self.previews = None;
        self.settings = None;
        self.area = None;
        self.area_asked = None;
        let seconds = self.store.get().countdown.seconds();
        self.session = if seconds == 0 {
            Session::Launch
        } else {
            Session::Countdown(Countdown::new(seconds, Instant::now()))
        };
        self.ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    pub fn cancel_countdown(&mut self) {
        if matches!(self.session, Session::Countdown(_)) {
            self.session = Session::Idle;
        }
    }

    /// Advances the countdown, starts the recorder and takes its events; every frame.
    pub fn tick(&mut self) {
        match &mut self.session {
            Session::Countdown(countdown) => {
                let now = Instant::now();
                if countdown.tick(now) {
                    self.session = Session::Launch;
                    self.ctx.request_repaint_of(egui::ViewportId::ROOT);
                } else {
                    // The main window: `tick` runs outside a pass, where the current viewport
                    // may be another window (the countdown's), and only the main one ticks.
                    self.ctx
                        .request_repaint_after_for(countdown.next - now, egui::ViewportId::ROOT);
                }
            }
            Session::Launch => {
                self.session = match self.start_recorder() {
                    Ok(recorder) => Session::Starting(recorder),
                    Err(error) => {
                        self.report(Err(error));
                        Session::Idle
                    }
                };
            }
            Session::Idle => {}
            Session::Starting(recorder)
            | Session::Live(Live { recorder, .. })
            | Session::Preparing(recorder)
            | Session::Cancelling(recorder) => {
                for event in recorder.events() {
                    self.on_event(event);
                }
            }
        }
    }

    fn start_recorder(&self) -> Result<Recorder, AppError> {
        let settings = self.store.get();
        let dir = &settings.recordings_dir;
        std::fs::create_dir_all(dir).map_err(|e| AppError::settings_file(dir, e))?;
        let root = dc_capture::project::free_project_root(dir);
        let defaults = dc_project::Defaults::load(&dc_project::settings::defaults_file()?)?;
        Recorder::start(&self.ctx, settings.recording.clone(), root, defaults)
    }

    fn on_event(&mut self, event: Event) {
        match (event, std::mem::take(&mut self.session)) {
            (Event::Started(start), Session::Starting(recorder)) => {
                self.session = Session::Live(Live {
                    recorder,
                    start,
                    pauses: Pauses::default(),
                    asking: false,
                });
            }
            (Event::Paused(at), Session::Live(mut live)) => {
                self.report(live.pauses.pause(at));
                self.session = Session::Live(live);
            }
            (Event::Resumed(at), Session::Live(mut live)) => {
                self.report(live.pauses.resume(at));
                self.session = Session::Live(live);
            }
            (Event::Error(message), session) => {
                self.session = session;
                self.report(Err(message));
            }
            (Event::Stopped(project), session) => {
                finished(session);
                self.recorded(*project);
            }
            (Event::Cancelled, session) => finished(session),
            (Event::Failed(message), Session::Preparing(recorder)) => {
                let root = recorder.root().to_path_buf();
                recorder.join();
                self.show_hud();
                self.failure = Some((message, root));
                self.popover = Some(popover::Open::new(popover::Kind::Failed, 0.0));
            }
            (Event::Failed(message), session) => {
                finished(session);
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
    /// the editor closes (v1).
    fn recorded(&mut self, project: dc_capture::win::RecordedProject) {
        tracing::info!("запись сохранена: {}", project.dir.root().display());
        self.open_editor(&project);
    }

    /// Pause or resume.
    pub fn toggle_pause(&mut self) {
        if let Session::Live(live) = &self.session {
            let request = if live.paused() {
                Request::Resume
            } else {
                Request::Pause
            };
            live.recorder.send(request);
        }
    }

    /// Stop: the HUD shows «Подготовка записи» until the project is saved.
    /// A countdown is cancelled; any other state stays as it is.
    pub fn stop(&mut self) {
        match std::mem::take(&mut self.session) {
            Session::Live(live) => {
                live.recorder.send(Request::Stop);
                self.popover = None;
                self.session = Session::Preparing(live.recorder);
                self.show_hud();
            }
            Session::Countdown(_) => {}
            other => self.session = other,
        }
    }

    /// ✕: pauses the recording and asks «Отменить запись?» over the button at `anchor`.
    pub fn ask_cancel(&mut self, anchor: f32) {
        if let Session::Live(live) = &mut self.session {
            if !live.paused() {
                live.recorder.send(Request::Pause);
            }
            live.asking = true;
            self.popover = Some(popover::Open::new(popover::Kind::Cancel, anchor));
        }
    }

    /// The answer to «Отменить запись?»: delete it, or go on recording (also Esc and a click
    /// elsewhere).
    pub fn answer_cancel(&mut self, delete: bool) {
        self.popover = None;
        match std::mem::take(&mut self.session) {
            Session::Live(live) if delete => {
                live.recorder.send(Request::Cancel);
                self.session = Session::Cancelling(live.recorder);
            }
            Session::Live(mut live) => {
                if live.paused() {
                    live.recorder.send(Request::Resume);
                }
                live.asking = false;
                self.session = Session::Live(live);
            }
            other => self.session = other,
        }
    }

    /// A global hotkey: start/stop starts at rest, cancels the countdown, stops a recording;
    /// pause and cancel work on a recording only.
    pub fn hotkey(&mut self, command: Command) {
        match (command, &self.session) {
            (Command::StartStop, Session::Idle) => self.record(),
            (Command::StartStop, _) => self.stop(),
            (Command::Pause, _) => self.toggle_pause(),
            (Command::Cancel, Session::Live(live)) if !live.asking => {
                self.show_hud();
                self.ask_cancel(0.0);
            }
            (Command::Cancel, Session::Countdown(_)) => self.cancel_countdown(),
            (Command::Cancel, _) => {}
        }
    }

    /// «Выход»: at once at rest; while recording the HUD asks «Сохранить и выйти?» first.
    pub fn quit(&mut self) {
        match self.session {
            Session::Starting(_) | Session::Live(_) => {
                self.show_hud();
                self.popover = Some(popover::Open::new(popover::Kind::Quit, 0.0));
            }
            _ => self
                .ctx
                .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close),
        }
    }

    /// The tray's status and «Остановить».
    pub fn tray_view(&self) -> TrayView {
        let source =
            || crate::hud::bar::source_label(&self.store.get().recording.source, &self.sources);
        match &self.session {
            Session::Live(live) => TrayView {
                status: tray::recording(&live.timer(), &source()),
                recording: true,
                can_stop: true,
            },
            Session::Preparing(_) => TrayView {
                status: dc_ui_kit::strings::hud::PREPARING.to_owned(),
                recording: true,
                can_stop: false,
            },
            Session::Starting(_) | Session::Cancelling(_) => TrayView {
                status: tray::recording("00:00", &source()),
                recording: true,
                can_stop: false,
            },
            Session::Idle | Session::Countdown(_) | Session::Launch => TrayView {
                status: tray::READY.to_owned(),
                recording: false,
                can_stop: matches!(self.session, Session::Countdown(_)),
            },
        }
    }

    /// «Скрыть панель»: the tray's «Показать панель» brings it back; hotkeys work meanwhile.
    pub fn hide_hud(&self) {
        self.ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::Visible(false),
        );
    }

    pub fn show_hud(&self) {
        self.ctx
            .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Visible(true));
    }
}

/// Joins the thread of a recorder that has sent its last event.
fn finished(session: Session) {
    match session {
        Session::Starting(recorder)
        | Session::Live(Live { recorder, .. })
        | Session::Preparing(recorder)
        | Session::Cancelling(recorder) => recorder.join(),
        Session::Idle | Session::Countdown(_) | Session::Launch => {}
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
