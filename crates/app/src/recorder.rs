//! The recording of the app on its own thread: the thread starts `ProjectRecording` (as
//! `devcast record` does), then serves pause, resume, stop and cancel; the UI never waits for
//! the recorder. Each event wakes the UI. When the app goes away without a stop, the recording
//! is stopped and kept, as `devcast record` does when its channel closes.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use dc_capture::win::{ProjectRecording, RecordedProject};
use dc_capture::{HostTime, RecordingConfig};
use dc_project::Defaults;
use eframe::egui;

use crate::error::AppError;

/// Who holds the project lock while recording.
const LOCK_OWNER: &str = "devcast-app";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Pause,
    Resume,
    Stop,
    Cancel,
}

#[derive(Debug)]
pub enum Event {
    /// The first frame is at this host time.
    Started(HostTime),
    Paused(HostTime),
    Resumed(HostTime),
    /// A pause or resume failed; the recording goes on.
    Error(String),
    /// The recording is finished and its project saved; the thread has ended.
    Stopped(Box<RecordedProject>),
    /// The recording and its project are deleted; the thread has ended.
    Cancelled,
    /// The recording did not start or did not finish; the thread has ended.
    Failed(String),
}

pub struct Recorder {
    requests: Sender<Request>,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
}

impl Recorder {
    /// Starts recording `config` into the new project folder `root`.
    pub fn start(
        ctx: &egui::Context,
        config: RecordingConfig,
        root: PathBuf,
        defaults: Defaults,
    ) -> Result<Self, AppError> {
        let (requests, received) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let ctx = ctx.clone();
        let thread = std::thread::Builder::new()
            .name("devcast-recorder".to_owned())
            .spawn(move || {
                let send = |event| {
                    // The UI is gone only when the app quits; then nobody needs the event.
                    let _ = sender.send(event);
                    ctx.request_repaint();
                };
                run(&config, &root, defaults, &received, send);
            })
            .map_err(|source| AppError::Thread {
                name: "запись",
                source,
            })?;
        Ok(Self {
            requests,
            events,
            thread: Some(thread),
        })
    }

    pub fn send(&self, request: Request) {
        if self.requests.send(request).is_err() {
            tracing::warn!("запись уже закончилась, {request:?} не отправлено");
        }
    }

    /// Events that came since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    /// Waits until the recording thread has ended (after a stop or cancel was sent).
    pub fn join(mut self) {
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("поток записи упал");
        }
    }
}

fn run(
    config: &RecordingConfig,
    root: &std::path::Path,
    defaults: Defaults,
    requests: &Receiver<Request>,
    send: impl Fn(Event),
) {
    let recording = match ProjectRecording::start(config, root, LOCK_OWNER, defaults) {
        Ok(recording) => recording,
        Err(error) => return send(Event::Failed(error.to_string())),
    };
    send(Event::Started(recording.recording().info().start));
    loop {
        // The app went away without a stop: the recording is kept.
        let request = requests.recv().unwrap_or(Request::Stop);
        match request {
            Request::Pause => send(match recording.pause() {
                Ok(at) => Event::Paused(at),
                Err(error) => Event::Error(error.to_string()),
            }),
            Request::Resume => send(match recording.resume() {
                Ok(at) => Event::Resumed(at),
                Err(error) => Event::Error(error.to_string()),
            }),
            Request::Stop => {
                return send(match recording.stop() {
                    Ok(project) => Event::Stopped(Box::new(project)),
                    Err(error) => Event::Failed(error.to_string()),
                });
            }
            Request::Cancel => {
                return send(match recording.cancel() {
                    Ok(()) => Event::Cancelled,
                    Err(error) => Event::Failed(error.to_string()),
                });
            }
        }
    }
}
