//! The fields of a blur annotation, kept in `Annotation::body` (fx/annotations).

use dc_fx_annotations::Blend;
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strength of the blur: σ of the gaussian, px when the short side of the frame is 1080 px (v1).
pub const STRENGTH_MIN: f64 = 1.0;
pub const STRENGTH_MAX: f64 = 100.0;
/// Corner radius, px when the short side of the frame is 1080 px.
pub const RADIUS_MAX: f64 = 540.0;

/// How a blur annotation looks; a missing field takes its default (v1: σ 20, no tint, corners 8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct BlurBody {
    /// Strength of the blur: σ of the gaussian, px when the short side of the frame is 1080 px,
    /// 1..100; it does not grow with the box.
    pub strength: f64,
    /// Tint over the blur; its alpha is how much it covers: 00 is a pure blur, ff a solid
    /// «censor» fill (decision 000).
    pub tint: Rgba,
    /// Corner radius, px when the short side of the frame is 1080 px, 0..540 (decision 000:
    /// «Тонкая настройка», 8 by default).
    pub radius: f64,
}

impl Default for BlurBody {
    fn default() -> Self {
        Self {
            strength: 20.0,
            tint: Rgba([0, 0, 0, 0]),
            radius: 8.0,
        }
    }
}

/// Between two keys every field blends: numbers linearly, the tint by channel (so its
/// opacity fades too).
impl Blend for BlurBody {
    fn blend(&self, to: &Self, e: f64) -> Self {
        Self {
            strength: self.strength.blend(&to.strength, e),
            tint: self.tint.blend(&to.tint, e),
            radius: self.radius.blend(&to.radius, e),
        }
    }
}
