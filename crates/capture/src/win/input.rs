//! Input of a recording: the hook thread and the writer thread (decision 009), which saves
//! cursor pictures as PNG.

use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use super::cursor::encode_png;
use super::hooks::Hooks;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::input::{InputMessage, InputSummary, write};

pub struct InputRecording {
    hooks: Option<Hooks>,
    events: Option<Sender<InputMessage>>,
    writer: Option<JoinHandle<Result<InputSummary, CaptureError>>>,
}

impl InputRecording {
    /// Installs the hooks; events are kept from [`Self::begin`].
    pub fn start() -> Result<Self, CaptureError> {
        let (events, messages) = mpsc::channel();
        let writer = std::thread::Builder::new()
            .name("devcast-input-writer".to_owned())
            .spawn(move || write(&messages, encode_png))
            .map_err(|e| {
                CaptureError::platform("запустить запись мыши")(e.to_string())
            })?;
        let hooks = Hooks::start(events.clone())?;
        Ok(Self {
            hooks: Some(hooks),
            events: Some(events),
            writer: Some(writer),
        })
    }

    /// Input is recorded from host time `start` into the folder `dir`.
    pub fn begin(&self, start: HostTime, dir: &Path) -> Result<(), CaptureError> {
        let message = InputMessage::Begin(start, dir.to_path_buf());
        self.events
            .as_ref()
            .ok_or_else(Self::gone)?
            .send(message)
            .map_err(|_| Self::gone())
    }

    /// Removes the hooks: input from now on is not recorded. Called before the stop time of
    /// the recording is taken, so every event is earlier than it.
    pub fn stop(&mut self) {
        self.hooks = None;
        self.events = None;
    }

    /// Waits until the writer has written everything; call after [`Self::stop`].
    pub fn join(mut self) -> Result<InputSummary, CaptureError> {
        self.stop();
        let writer = self.writer.take().ok_or_else(Self::gone)?;
        writer.join().map_err(|_| Self::gone())?
    }

    fn gone() -> CaptureError {
        CaptureError::platform("записать мышь")("поток записи ввода упал".to_owned())
    }
}

impl Drop for InputRecording {
    /// Not stopped (the recording failed to start or is cancelled): the writer ends after
    /// writing what came.
    fn drop(&mut self) {
        self.stop();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}
