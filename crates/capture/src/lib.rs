//! Recorder trait, segmented recording with manifest, crash recovery, platform capture backends.

pub mod audio;
pub mod cancel;
pub mod config;
pub mod error;
pub mod event;
pub mod finalize;
pub mod grid;
pub mod input;
pub mod lead;
pub mod list;
pub mod manifest;
pub mod pause;
pub mod preview;
pub mod recorder;
pub mod recover;
pub mod source;
pub mod track;
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "Win32 and Direct3D 11 calls; each block has a SAFETY comment"
)]
pub mod win;

pub use cancel::remove_recording;
pub use config::{ConfigError, Fps, RecordingConfig};
pub use error::CaptureError;
pub use event::{HostTime, RecordingEvent};
pub use finalize::finalize;
pub use grid::FrameGrid;
pub use lead::{FrameLead, LeadFilter};
pub use list::{MonitorInfo, SourceList, WindowInfo};
pub use manifest::{Manifest, Segment, SharedManifest, Track};
pub use pause::{Pause, PauseClock, Pauses, Placed};
pub use preview::{BgraImage, Preview};
pub use recorder::Recorder;
pub use recover::recover;
pub use source::{Area, Source, SourceError};
