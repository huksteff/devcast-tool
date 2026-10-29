//! Region blur: what is painted below a rectangle, seen through frosted glass.

use serde::{Deserialize, Serialize};

use super::{PxRect, Rgba};

/// Everything painted before it inside `rect` (corners rounded by `radius`), blurred by a
/// gaussian of `sigma` px of the frame and mixed with `tint` by the tint's alpha: 0 is a pure
/// blur, 255 a solid «censor» fill (v1 blur annotation, decision 000).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blur {
    pub rect: PxRect,
    pub radius: f32,
    pub sigma: f32,
    pub tint: Rgba,
}
