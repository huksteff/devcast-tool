//! Sound tracks of a recording through cpal (WASAPI on Windows): system sound (loopback of
//! the default output) and the microphone, on the host clock like the video (decision 008).

mod device;
mod drift;
mod timeline;
mod track;

pub use device::{AudioSource, CHANNELS, MicrophoneInfo, list_microphones};
pub use timeline::{AudioTimeline, RATE, TimelineStats};
pub use track::{AudioRecording, AudioSummary, SYSTEM_TRACK};
