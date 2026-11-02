//! The recording of the app on its own thread: the thread starts `ProjectRecording` (as
//! `devcast record` does), then serves pause, resume, stop and cancel; the UI never waits for
//! the recorder. Each event wakes the UI. After the stop the project gets its auto zooms, as after
//! `devcast record`. When the app goes away without a stop, the recording is stopped and kept, as
//! `devcast record` does when its channel closes.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use dc_capture::win::{ProjectRecording, RecordedProject};
use dc_capture::{HostTime, RecordingConfig};
use dc_project::{Defaults, ProjectDir, ProjectLock};
use eframe::egui;

use crate::editor::LOCK_OWNER;
use crate::error::AppError;

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
    /// The recording is finished and its project saved with its auto zooms; the thread has
    /// ended. The second field is why the auto zooms were not added.
    Stopped(Box<RecordedProject>, Option<String>),
    /// The recording and its project are deleted; the thread has ended.
    Cancelled,
    /// The recording did not start or did not finish; the thread has ended.
    Failed(String),
}

pub struct Recorder {
    /// The project folder being recorded into.
    root: PathBuf,
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
        let (ctx, folder) = (ctx.clone(), root.clone());
        let thread = std::thread::Builder::new()
            .name("devcast-recorder".to_owned())
            .spawn(move || {
                let send = |event| {
                    // The UI is gone only when the app quits; then nobody needs the event.
                    let _ = sender.send(event);
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                };
                run(&config, &root, defaults, &received, send);
            })
            .map_err(|source| AppError::Thread {
                name: "запись",
                source,
            })?;
        Ok(Self {
            root: folder,
            requests,
            events,
            thread: Some(thread),
        })
    }

    /// A recorder of `root` without a thread (tests): the test sends its events and reads the
    /// requests it got.
    #[cfg(test)]
    pub(crate) fn fake(root: PathBuf) -> (Self, Sender<Event>, Receiver<Request>) {
        let (requests, received) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let recorder = Self {
            root,
            requests,
            events,
            thread: None,
        };
        (recorder, sender, received)
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
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
                    Ok(project) => {
                        let zooms = auto_zooms(&project.dir).err();
                        Event::Stopped(Box::new(project), zooms)
                    }
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

/// The auto zooms of the recording just stopped into `dir` (`dc_ops::autozoom::after_recording`,
/// the path of `devcast record`); the error says why they were not added.
pub(crate) fn auto_zooms(dir: &ProjectDir) -> Result<(), String> {
    let lock = ProjectLock::acquire(dir, LOCK_OWNER).map_err(|e| e.to_string())?;
    let applied = dc_ops::autozoom::after_recording(dir, &lock).map_err(|e| e.to_string())?;
    if let Some(applied) = applied {
        tracing::info!("автозумы после записи: {applied}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
