use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Aspect of the output frame. Clips of another aspect are fitted into it with black bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum Aspect {
    /// The crop frame of the first clip (default, like Cap's "Auto").
    #[default]
    #[serde(rename = "free")]
    Free,
    /// The source frame of the first clip.
    #[serde(rename = "source")]
    Source,
    /// 16:9.
    #[serde(rename = "16:9")]
    Wide16x9,
    /// 16:10.
    #[serde(rename = "16:10")]
    Wide16x10,
    /// 4:3.
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
    /// Aspect of the output frame; free by default.
    #[serde(default)]
    pub aspect: Aspect,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CropState {
    /// Output frame settings.
    #[serde(default)]
    pub settings: CropSettings,
}
