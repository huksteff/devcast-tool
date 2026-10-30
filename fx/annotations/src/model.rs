//! Annotations as stored in `project.effects["annotations"]`: the part every kind shares (time,
//! track, box, rotation) and the kind's own fields, which belong to the kind's crate.

use dc_core::time::Source;
use dc_core::{Effect, Id, IdCounter, IdKind, Item, Range, Rect, SourceTime, TimeDelta};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::ser::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::keys::Key;
use crate::motion::Stage;

/// Number of annotation tracks: `track` is `0..TRACKS`.
pub const TRACKS: u32 = 32;
/// Shortest annotation on the output video (docs/porting/annotations.md §10).
pub const MIN_DURATION: TimeDelta = TimeDelta::from_millis(100);
/// Box sides, fractions of the frame: 1 % (v1 resize minimum) to ten frames.
pub const SIZE_MIN: f64 = 0.01;
pub const SIZE_MAX: f64 = 10.0;
/// Box corner, fractions of the frame: the box may sit partly or fully off the frame.
pub const POSITION_LIMIT: f64 = 10.0;
/// Rotation, degrees either way: a full turn.
pub const ROTATION_LIMIT: f64 = 360.0;

pub struct Annotations;

impl Effect for Annotations {
    const ID: &'static str = "annotations";
    const TITLE: &'static str = "Аннотации";
    type State = AnnotationsState;
    const LISTS: &'static [&'static str] = &[AnnotKind::LIST];

    fn items(state: &AnnotationsState) -> Result<Vec<Item>, serde_json::Error> {
        state
            .annotations
            .iter()
            .map(|a| Item::of(a.id, &a.media, a.range, a))
            .collect()
    }
}

/// Id kind of annotations (`a1`, `a2`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AnnotKind;

impl IdKind for AnnotKind {
    const PREFIX: &'static str = "a";
    const NOUN: &'static str = "аннотации";
    const PLURAL: &'static str = "аннотации";
    const LIST: &'static str = "annots";
}

pub type AnnotId = Id<AnnotKind>;

/// What an annotation shows; each kind is drawn and edited by its own crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Text on an optional plate.
    Text,
    /// Arrow, rectangle, ellipse, circle, line or highlight.
    Shape,
    /// Picture from the project's `assets/`.
    Image,
    /// Pencil strokes.
    Drawing,
    /// Blurred and tinted area.
    Blur,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Shape => "shape",
            Self::Image => "image",
            Self::Drawing => "drawing",
            Self::Blur => "blur",
        }
    }
}

/// An annotation on the track of one recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    /// Id, `a<number>`.
    pub id: AnnotId,
    /// What it shows; its fields are in `body`.
    pub kind: Kind,
    /// Recording (`Media.path`) whose source time `range` is in: cutting and splitting clips
    /// keep the annotation on the same frames.
    pub media: String,
    /// Shown for `start <= t < end`, source time of the recording.
    pub range: Range<Source>,
    /// `0..TRACKS`; a higher track is painted above a lower one.
    pub track: u32,
    /// Box in the project frame before the zoom camera (decision 000): top-left corner and size,
    /// fractions of the frame width (x, w) and height (y, h). The annotation moves and scales
    /// with the camera.
    pub rect: Rect,
    /// Degrees clockwise around the box centre, `-ROTATION_LIMIT..=ROTATION_LIMIT`.
    pub rotation: f64,
    /// How it comes in: `none` by default.
    #[serde(default, skip_serializing_if = "Stage::is_default")]
    pub enter: Stage,
    /// How it goes out, played backwards from the end: `none` by default.
    #[serde(default, skip_serializing_if = "Stage::is_default")]
    pub exit: Stage,
    /// Fields of the kind, described by the kind's commands (`devcast help`); a missing field
    /// takes the kind's default. With keys this is what shows once the keys are gone.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub body: Map<String, Value>,
    /// Keyframes by time: with any key the box, rotation and kind fields come from the keys
    /// (blended between them), not from `rect`, `rotation` and `body`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Key>,
}

impl Annotation {
    /// The kind's fields as its type `B`, whose missing fields take defaults
    /// (`#[serde(default)]`).
    pub fn body<B: DeserializeOwned>(&self) -> Result<B, serde_json::Error> {
        serde_json::from_value(Value::Object(self.body.clone()))
    }

    /// Stores the kind's fields; `B` must serialize to a JSON object.
    #[cfg(test)]
    pub(crate) fn set_body<B: Serialize>(&mut self, body: &B) -> Result<(), serde_json::Error> {
        self.body = fields(body)?;
        Ok(())
    }

    /// Whether `other` is on screen at the same time: same recording, ranges overlap.
    pub fn meets(&self, other: &Self) -> bool {
        self.media == other.media && self.range.intersect(&other.range).is_some()
    }
}

/// The kind's fields `body` as stored; `B` must serialize to a JSON object.
pub(crate) fn fields<B: Serialize>(body: &B) -> Result<Map<String, Value>, serde_json::Error> {
    match serde_json::to_value(body)? {
        Value::Object(fields) => Ok(fields),
        other => Err(serde_json::Error::custom(format!(
            "поля вида аннотации должны быть JSON-объектом, а не {other}"
        ))),
    }
}

/// Everything the annotations store in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AnnotationsState {
    /// Within a track a later annotation is painted above an earlier one; `annot layer`
    /// changes this order. Overlapping in time is allowed on any track.
    pub annotations: Vec<Annotation>,
    /// Next free annotation id; numbers are never reused.
    pub ids: IdCounter<AnnotKind>,
}

impl AnnotationsState {
    pub fn annot_ids(&self) -> impl Iterator<Item = AnnotId> + '_ {
        self.annotations.iter().map(|a| a.id)
    }

    /// Annotations of recording `media` shown at its source moment `t`, bottom to top: by track,
    /// then by order within the track.
    pub fn shown_at(&self, media: &str, t: SourceTime) -> Vec<&Annotation> {
        let mut shown: Vec<&Annotation> = self
            .annotations
            .iter()
            .filter(|a| a.media == media && a.range.contains(t))
            .collect();
        shown.sort_by_key(|a| a.track);
        shown
    }
}
