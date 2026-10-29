use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SIZE_MIN: f64 = 0.5;
pub const SIZE_MAX: f64 = 10.0;

/// How the drawn cursor follows the recorded one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Smoothing {
    /// Exactly as recorded.
    Exact,
    /// The spring of the Focused motion preset: quick, with a slight overshoot.
    #[default]
    Focused,
    /// The spring of the Smooth motion preset: soft, without overshoot.
    Smooth,
}

impl Smoothing {
    pub const ALL: [Self; 3] = [Self::Exact, Self::Focused, Self::Smooth];

    pub fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Focused => "focused",
            Self::Smooth => "smooth",
        }
    }
}

/// Look and motion of the cursor of every recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CursorSettings {
    /// Draw the cursor; click effects are drawn either way.
    pub show: bool,
    /// Size, `SIZE_MIN..=SIZE_MAX`: 1 draws the arrow 28 px tall in a clip 1920 px wide.
    pub size: f64,
    pub smoothing: Smoothing,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            show: true,
            size: 2.5,
            smoothing: Smoothing::Focused,
        }
    }
}

/// Everything the cursor effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CursorState {
    pub settings: CursorSettings,
}
