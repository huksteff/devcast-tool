//! Settings of click effects: one style for every click and changes of single clicks
//! (docs/porting/clicks-callout.md §1–§9).

use dc_core::{SourceTime, TimeDelta};
use dc_render::{BLUE, Rgba};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CLICK_SCALE_MIN: f64 = 0.5;
pub const CLICK_SCALE_MAX: f64 = 2.0;
pub const CLICK_TIME_MIN: TimeDelta = TimeDelta::from_millis(120);
pub const CLICK_TIME_MAX: TimeDelta = TimeDelta::from_millis(1200);
/// A changed click is the press of its recording at most this far from the stored moment (the
/// moment comes from the output time the user typed, rounded to 1 ms of output and more of the
/// recording on sped-up parts).
pub const CLICK_MATCH: TimeDelta = TimeDelta::from_millis(50);

/// What a click looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClickKind {
    /// No effect.
    #[default]
    Off,
    /// One ring growing out of the point.
    // `spotlight` and `echo` were removed as too alike to it (owner, 2026-11-12); older
    // projects, defaults and history still hold them.
    #[serde(alias = "spotlight", alias = "echo")]
    Ripple,
    /// A soft flash with 18 particles.
    Flash,
}

impl ClickKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Ripple => "ripple",
            Self::Flash => "flash",
        }
    }
}

/// The effect of every click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ClickStyle {
    /// What a click looks like; off by default.
    pub kind: ClickKind,
    /// Colour of the effect, #rrggbbaa.
    pub color: Rgba,
    /// Size, `CLICK_SCALE_MIN..=CLICK_SCALE_MAX`, × the cursor arrow height.
    pub scale: f64,
    /// 0..1.
    pub opacity: f64,
    /// On the output timeline, `CLICK_TIME_MIN..=CLICK_TIME_MAX`; starts half a bounce after the
    /// press, when the cursor is squeezed most.
    pub duration: TimeDelta,
}

impl Default for ClickStyle {
    fn default() -> Self {
        Self {
            kind: ClickKind::Off,
            color: BLUE,
            scale: 1.0,
            opacity: 1.0,
            duration: TimeDelta::from_millis(600),
        }
    }
}

/// One click shown otherwise than the rest; empty fields come from `ClickStyle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClickChange {
    /// Recording (`Media.path`) of the click.
    pub media: String,
    /// The press, on the recording's time (`CLICK_MATCH` around it).
    pub at: SourceTime,
    /// `off` hides the effect of this click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ClickKind>,
    /// Colour of the effect, #rrggbbaa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    /// Size, `CLICK_SCALE_MIN..=CLICK_SCALE_MAX`, × the cursor arrow height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// 0..1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// On the output timeline, `CLICK_TIME_MIN..=CLICK_TIME_MAX`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<TimeDelta>,
}

impl ClickChange {
    /// `style` with this click's own fields.
    pub fn applied_to(&self, style: &ClickStyle) -> ClickStyle {
        ClickStyle {
            kind: self.kind.unwrap_or(style.kind),
            color: self.color.unwrap_or(style.color),
            scale: self.scale.unwrap_or(style.scale),
            opacity: self.opacity.unwrap_or(style.opacity),
            duration: self.duration.unwrap_or(style.duration),
        }
    }

    /// Whether this changes the press of `media` at `at`.
    pub fn is_for(&self, media: &str, at: SourceTime) -> bool {
        self.media == media && at.since(self.at).micros().abs() <= CLICK_MATCH.micros()
    }
}
