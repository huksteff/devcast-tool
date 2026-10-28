//! Zoom regions and motion settings as stored in `project.effects["zoom"]`.

use dc_core::time::Source;
use dc_core::{ClipId, Effect, Id, IdCounter, IdKind, Point, Range, TimeDelta};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SCALE_MIN: f64 = 1.0;
pub const SCALE_MAX: f64 = 5.0;
pub const DEFAULT_SCALE: f64 = 1.5;
/// Zoom-in and zoom-out durations allowed by the settings, as in v1.
pub const TRANSITION_MIN: TimeDelta = TimeDelta::from_millis(60);
pub const TRANSITION_MAX: TimeDelta = TimeDelta::from_millis(4000);
pub const MOTION_BLUR_MAX: f64 = 2.0;

/// The zoom effect.
pub struct ZoomEffect;

impl Effect for ZoomEffect {
    const ID: &'static str = "zoom";
    const TITLE: &'static str = "Зум";
    type State = ZoomState;
}

/// Id kind of zoom regions (`z1`, `z2`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ZoomKind;

impl IdKind for ZoomKind {
    const PREFIX: &'static str = "z";
    const NOUN: &'static str = "зума";
    const PLURAL: &'static str = "зумы";
    const LIST: &'static str = "ls zooms";
}

pub type ZoomId = Id<ZoomKind>;

/// Where the camera looks while zoomed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Follows the cursor, starting from `focus`.
    #[default]
    Follow,
    /// Stays on `focus`.
    Fixed,
}

/// A zoom region of one clip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Zoom {
    pub id: ZoomId,
    /// Clip whose source time `range` is in.
    pub clip: ClipId,
    pub range: Range<Source>,
    /// Magnification, `SCALE_MIN..=SCALE_MAX`.
    pub scale: f64,
    /// Point of the project frame (before the camera) to zoom into, 0..1 on both axes.
    pub focus: Point,
    pub mode: Mode,
}

/// Camera motion of all zooms of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ZoomSettings {
    /// Zoom-in duration on the output timeline, `TRANSITION_MIN..=TRANSITION_MAX`.
    pub zoom_in: TimeDelta,
    /// Zoom-out duration on the output timeline, `TRANSITION_MIN..=TRANSITION_MAX`.
    pub zoom_out: TimeDelta,
    /// Camera smoothness slider: 0 sharp, 1 soft. Tunes the camera spring on top of the preset.
    pub smoothness: f64,
    /// Zoom motion blur strength, `0..=MOTION_BLUR_MAX`; 0 turns it off.
    pub motion_blur: f64,
    /// Close zooms hand over to each other without zooming out in between.
    pub connect: bool,
}

/// Ready-made zoom-in and zoom-out durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Quick zooms for demo videos.
    Focused,
    /// Soft Keynote-style zooms.
    Smooth,
}

impl Preset {
    pub const ALL: [Self; 2] = [Self::Focused, Self::Smooth];

    pub fn title(self) -> &'static str {
        match self {
            Self::Focused => "Focused",
            Self::Smooth => "Smooth",
        }
    }

    /// `(zoom_in, zoom_out)`.
    pub fn durations(self) -> (TimeDelta, TimeDelta) {
        match self {
            Self::Focused => (TimeDelta::from_millis(200), TimeDelta::from_millis(200)),
            Self::Smooth => (
                TimeDelta::from_micros(1_522_575),
                TimeDelta::from_micros(1_015_050),
            ),
        }
    }
}

impl Default for ZoomSettings {
    fn default() -> Self {
        let (zoom_in, zoom_out) = Preset::Focused.durations();
        Self {
            zoom_in,
            zoom_out,
            smoothness: 0.5,
            motion_blur: 0.35,
            connect: true,
        }
    }
}

impl ZoomSettings {
    /// The preset whose durations these settings have, if any.
    pub fn preset(&self) -> Option<Preset> {
        Preset::ALL
            .into_iter()
            .find(|p| p.durations() == (self.zoom_in, self.zoom_out))
    }
}

/// Everything the zoom effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ZoomState {
    /// In the order they were added; overlapping is not allowed within a clip.
    pub zooms: Vec<Zoom>,
    pub ids: IdCounter<ZoomKind>,
    pub settings: ZoomSettings,
}

impl ZoomState {
    pub fn zoom_ids(&self) -> impl Iterator<Item = ZoomId> + '_ {
        self.zooms.iter().map(|z| z.id)
    }
}
