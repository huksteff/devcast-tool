//! Motion blur (ARCHITECTURE: several samples along the velocity).

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// The layer averaged over `samples` moments of the shutter, evenly spread from where it was at
/// the opening (`from`) to where it is now, both ends included; 1 sample shows it sharp where
/// it is. Evaluate picks `from` (v1: the motion of the last 1/60 s scaled by the blur strength).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Motion<T> {
    pub from: T,
    pub samples: NonZeroU32,
}

impl<T> Motion<T> {
    /// How far from now each sample is, as a part of the way back to `from`: sample `i` is at
    /// `(samples - 1 - i) * step`, so the last one is exactly now.
    pub(crate) fn step(&self) -> f32 {
        match self.samples.get() {
            1 => 0.0,
            n => 1.0 / (n - 1) as f32,
        }
    }
}
