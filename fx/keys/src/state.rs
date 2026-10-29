use dc_core::TimeDelta;
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SIZE_MIN: f64 = 0.5;
pub const SIZE_MAX: f64 = 3.0;
pub const DURATION_MIN: TimeDelta = TimeDelta::from_millis(300);
pub const DURATION_MAX: TimeDelta = TimeDelta::from_millis(5000);

/// Where the plate stands in the output frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    TopLeft,
    Top,
    TopRight,
    BottomLeft,
    #[default]
    Bottom,
    BottomRight,
}

impl Position {
    pub fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::Top => "top",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom_right",
        }
    }

    pub(crate) fn top(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }
}

/// Look of the pressed-keys plate of every recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct KeysSettings {
    /// Show the shortcuts pressed during the recordings; off by default.
    pub show: bool,
    pub position: Position,
    /// Size, `SIZE_MIN..=SIZE_MAX`: 1 is a 36 px font when the short side of the frame is 1080 px.
    pub size: f64,
    /// Colour of the letters.
    pub color: Rgba,
    /// Colour of the plate behind them.
    pub background: Rgba,
    /// How long a shortcut stays after its last press, on the output timeline,
    /// `DURATION_MIN..=DURATION_MAX`. The same shortcut pressed again while shown is counted
    /// (`Ctrl+Z ×3`) instead of shown anew.
    pub duration: TimeDelta,
}

impl Default for KeysSettings {
    fn default() -> Self {
        Self {
            show: false,
            position: Position::Bottom,
            size: 1.0,
            color: Rgba([0xff, 0xff, 0xff, 0xff]),
            background: Rgba([0x11, 0x18, 0x27, 0xd9]),
            duration: TimeDelta::from_millis(1500),
        }
    }
}

/// Everything the keys effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct KeysState {
    pub settings: KeysSettings,
}
