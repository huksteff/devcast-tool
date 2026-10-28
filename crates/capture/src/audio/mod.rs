//! Sound tracks of a recording through cpal (WASAPI on Windows): system sound (loopback of
//! the default output) and the microphone, on the host clock like the video (decision 008).

mod device;
mod drift;
mod level;
mod timeline;
mod track;

pub use device::{AudioSource, CHANNELS, MicrophoneInfo, list_microphones};
pub use level::{AudioLevel, FLOOR_DB, LEVELS_PER_SECOND, LevelMeter};
pub use timeline::{AudioTimeline, RATE, TimelineStats};
pub use track::{AudioRecording, AudioSummary, MIC_TRACK, SYSTEM_TRACK};
