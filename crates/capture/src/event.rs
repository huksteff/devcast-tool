//! The host clock shared by video, audio and input.

use serde::{Deserialize, Serialize};

/// Time on the monotonic system clock shared by video, audio and input (QPC on Windows),
/// in microseconds since an arbitrary start (boot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostTime(i64);

impl HostTime {
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    pub const fn micros(self) -> i64 {
        self.0
    }
}
