//! The platform-independent recording interface.

use std::sync::mpsc::Sender;

use crate::config::{ConfigError, RecordingConfig};
use crate::event::RecordingEvent;

/// A running recording. Each platform implements it; callers see only this trait.
///
/// State changes and failures during recording arrive as [`RecordingEvent`]s on the channel
/// given to [`Recorder::start`]; a failed call means nothing changed.
pub trait Recorder: Sized {
    /// Validates `config` and starts recording; sends [`RecordingEvent::Started`].
    fn start(config: RecordingConfig, events: Sender<RecordingEvent>)
    -> Result<Self, CaptureError>;

    /// Stops writing until [`Recorder::resume`]; the gap is cut out of the recording.
    fn pause(&mut self) -> Result<(), CaptureError>;

    fn resume(&mut self) -> Result<(), CaptureError>;

    /// Finishes the recording and its files.
    fn stop(self) -> Result<(), CaptureError>;

    /// Stops and deletes everything recorded.
    fn cancel(self) -> Result<(), CaptureError>;
}

/// Why recording could not start or continue. Every message says what to do.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error(transparent)]
    Config(#[from] ConfigError),
}
