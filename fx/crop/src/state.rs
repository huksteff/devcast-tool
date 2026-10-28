use std::collections::BTreeMap;
use std::fmt;

use dc_core::{ClipId, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Smallest crop width and height, as a fraction of the source frame.
pub const MIN_SIZE: f64 = 0.05;

/// Aspect of the output frame. Clips of another aspect are fitted into it with black bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum Aspect {
    /// The crop frame of the first clip.
    #[serde(rename = "free")]
    Free,
    /// The source frame of the first clip.
    #[default]
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "16:9")]
    Wide16x9,
    #[serde(rename = "16:10")]
    Wide16x10,
    #[serde(rename = "4:3")]
    Standard4x3,
}

impl fmt::Display for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Free => "free",
            Self::Source => "source",
            Self::Wide16x9 => "16:9",
            Self::Wide16x10 => "16:10",
            Self::Standard4x3 => "4:3",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CropSettings {
    #[serde(default)]
    pub aspect: Aspect,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CropState {
    #[serde(default)]
    pub settings: CropSettings,
    /// Visible part of each cropped clip, as fractions of its source frame. A clip without an
    /// entry is shown whole. Entries of removed clips stay: ids are never reused, and undoing the
    /// removal brings the clip back with its crop.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub clips: BTreeMap<ClipId, Rect>,
}
