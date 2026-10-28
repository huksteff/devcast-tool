//! Recorder trait, segmented recording with manifest, crash recovery, platform capture backends.

pub mod config;
pub mod event;
pub mod recorder;
pub mod source;

pub use config::{ConfigError, Fps, RecordingConfig};
pub use event::{HostTime, RecordingEvent};
pub use recorder::{CaptureError, Recorder};
pub use source::{Area, Source, SourceError};
