//! Sprite layer: a picture (image annotation, cursor) drawn into a rectangle.

use serde::{Deserialize, Serialize};

use super::PxRect;

/// The whole picture stretched over `rect` (evaluate keeps its proportions), turned by
/// `rotation` degrees clockwise around the centre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    pub image: ImageKey,
    pub rect: PxRect,
    pub rotation: f32,
    /// 0..1.
    pub opacity: f32,
}

/// Which uploaded picture a sprite shows: a file of the project and a frame of its animation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageKey {
    /// Path of the picture in the project (`assets/logo.png`, `media/recording-1/cursor-0003.png`).
    pub path: String,
    /// Frame of an animated picture; 0 for a still one.
    pub frame: u32,
}
