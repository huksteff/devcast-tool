use dc_core::TimeDelta;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::click_state::{ClickChange, ClickStyle};

pub const SIZE_MIN: f64 = 0.5;
pub const SIZE_MAX: f64 = 10.0;
pub const BLUR_MAX: f64 = 2.0;
pub const BOUNCE_MAX: f64 = 5.0;
pub const BOUNCE_TIME_MIN: TimeDelta = TimeDelta::from_millis(60);
pub const BOUNCE_TIME_MAX: TimeDelta = TimeDelta::from_millis(500);
pub const SWAY_MAX: f64 = 2.0;

/// How the drawn cursor follows the recorded one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Smoothing {
    /// Exactly as recorded.
    #[default]
    Exact,
    /// The spring of the Focused motion preset: quick, with a slight overshoot.
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
    /// How the drawn cursor follows the recorded one.
    pub smoothing: Smoothing,
    /// Motion blur, `0..=BLUR_MAX`: the trail of the last `blur / 30` s of output; 0 is off.
    pub blur: f64,
    /// How much a click squeezes the cursor, `0..=BOUNCE_MAX` (×8 % of its height, down to 72 %).
    pub bounce: f64,
    /// Length of the squeeze on the output timeline, `BOUNCE_TIME_MIN..=BOUNCE_TIME_MAX`.
    pub bounce_time: TimeDelta,
    /// Tilt in the direction of motion, `0..=SWAY_MAX`: up to `sway · 30°` at speed.
    pub sway: f64,
    /// The last 670 ms of the video bring the cursor back to where it starts, for a looped video.
    #[serde(rename = "loop")]
    pub looped: bool,
    /// The effect of every click; drawn even with the cursor hidden.
    pub click: ClickStyle,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            show: true,
            size: 1.0,
            smoothing: Smoothing::Exact,
            blur: 0.0,
            bounce: 0.0,
            bounce_time: TimeDelta::from_millis(350),
            sway: 0.0,
            looped: false,
            click: ClickStyle::default(),
        }
    }
}

/// Everything the cursor effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CursorState {
    /// Look and motion of the cursor and the click effects.
    pub settings: CursorSettings,
    /// Clicks shown otherwise than `settings.click`, at most one per press.
    pub clicks: Vec<ClickChange>,
}
