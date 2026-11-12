//! The rows of the timeline as a table (docs/ARCHITECTURE.md, «Дорожка эффекта»): what a row is
//! called, its colour and height, the button of its header, whether its regions may overlap,
//! what a click on its empty place adds and how the effect reads the row from the project
//! ([`Kind::fill`]). The timeline reads these facts and never asks which effect a row belongs
//! to; a new kind of row is a [`Kind`] here and its place in `Tracks::of`. The order of the rows
//! is the order `Tracks::of` adds them in (owner's decision 2026-11-02).

use std::fmt;

use dc_core::time::Output;
use dc_core::{OpName, Project, Range, StateError, Step, TimeMap};
use dc_ui_kit::lane;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{self, Hue};

use super::tracks::Least;
use crate::editor::{lanes, rows};

/// A kind of rows: one row, or one row per track (annotations, extra sound).
pub struct Kind {
    /// Names the kind in tests and logs.
    id: &'static str,
    /// The name of the row of track `0..`.
    name: fn(u32) -> String,
    pub hue: Hue,
    /// Points (mockup: clips 50, sound 36, the rest 28).
    pub height: f32,
    /// The button at the right of the header.
    pub button: Button,
    /// The kind has a row per track: regions move between its rows, a row is removed by ×.
    pub many: Option<Many>,
    /// The header drags to another row of the kind: the order of the rows is the layer order.
    pub lifts: bool,
    /// Regions of a row do not overlap.
    pub apart: bool,
    /// A new region lies inside the clip under it.
    pub in_clip: bool,
    /// What a click on an empty place of a row adds.
    pub adds: Adds,
    /// The effect's row read from the project: its regions and whether it is off, and why its state
    /// is broken (a problem of the rows). None: `Tracks::of` fills the row itself (clips, tracks).
    pub fill: Option<Fill>,
    /// What the ghost of a new region says instead of «+».
    pub ghost: Option<fn() -> String>,
}

/// How an effect reads its row from the project through the time map.
pub type Fill = fn(&Project, &TimeMap) -> (lane::Row, Option<StateError>);

/// The button of a row header.
#[derive(Clone, Copy)]
pub enum Button {
    None,
    /// The eye: the command with `--show` of the effect of the row.
    Shown(OpName),
    /// The sound of this track of the recordings (`audio channel --mute`).
    Mute(&'static str),
    /// × of a row of a kind with tracks.
    Remove,
    /// The crop dialog of the clips.
    Crop,
}

/// What a click on an empty place of a row adds.
#[derive(Clone, Copy)]
pub enum Adds {
    Nothing,
    /// The command that adds a region at this place of this track.
    Step(fn(Range<Output>, u32) -> Step),
    /// The menu of annotation kinds, then the annotation.
    Annotation,
    /// The dialog of a sound file, then the sound.
    Sound,
}

/// A kind with a row per track.
pub struct Many {
    /// How many rows of the kind show at least ([`Least`]).
    pub least: fn(&mut Least) -> &mut u32,
    /// The commands that remove a track and move the tracks above it one down.
    pub remove: fn(&Project, u32) -> Vec<Step>,
    /// The question before a row with this many regions is removed (the second argument is
    /// the label of the undo key).
    pub question: fn(usize, &str) -> String,
}

static CAPTIONS: Kind = Kind {
    id: "captions",
    name: |_| text::CAPTIONS.to_owned(),
    hue: tokens::HUE_CAPTIONS,
    button: Button::Shown(lanes::captions::SHOW),
    fill: Some(lanes::captions::row),
    ..PLAIN
};

static CLIPS: Kind = Kind {
    id: "clips",
    name: |_| text::CLIPS.to_owned(),
    hue: tokens::HUE_CLIP,
    height: 50.0,
    button: Button::Crop,
    ..PLAIN
};

static ZOOMS: Kind = Kind {
    id: "zooms",
    name: |_| text::ZOOMS.to_owned(),
    hue: tokens::HUE_ZOOM,
    apart: true,
    adds: Adds::Step(|range, _| lanes::zoom::added(range)),
    ghost: Some(lanes::zoom::ghost),
    fill: Some(lanes::zoom::row),
    ..PLAIN
};

/// Clicks and the callout.
static CURSOR: Kind = Kind {
    id: "cursor",
    name: |_| text::CURSOR.to_owned(),
    hue: tokens::HUE_CURSOR,
    adds: Adds::Step(|range, _| lanes::callout::added(range)),
    fill: Some(lanes::callout::row),
    ..PLAIN
};

static KEYS: Kind = Kind {
    id: "keys",
    name: |_| text::KEYS.to_owned(),
    hue: tokens::HUE_KEYS,
    button: Button::Shown(lanes::keys::SHOW),
    fill: Some(lanes::keys::row),
    ..PLAIN
};

/// Annotation tracks `0..TRACKS`; a higher one is painted above.
static ANNOTATIONS: Kind = Kind {
    id: "annotations",
    name: |track| text::annotations(track + 1),
    hue: tokens::HUE_ANNOTATIONS,
    button: Button::Remove,
    many: Some(Many {
        least: |least| &mut least.annotations,
        remove: rows::remove_annotations,
        question: text::remove_annotations,
    }),
    lifts: true,
    adds: Adds::Annotation,
    ..PLAIN
};

/// Microphone of the recordings.
static MIC: Kind = Kind {
    id: "mic",
    name: |_| text::MIC.to_owned(),
    button: Button::Mute(dc_capture::audio::MIC_TRACK),
    ..SOUND
};

/// System sound of the recordings.
static SYSTEM: Kind = Kind {
    id: "system",
    name: |_| text::SYSTEM.to_owned(),
    button: Button::Mute(dc_capture::audio::SYSTEM_TRACK),
    ..SOUND
};

/// Extra sound tracks `0..TRACKS`.
static AUDIO: Kind = Kind {
    id: "audio",
    name: |track| text::audio(track + 1),
    button: Button::Remove,
    many: Some(Many {
        least: |least| &mut least.audio,
        remove: rows::remove_sound,
        question: text::remove_sound,
    }),
    apart: true,
    in_clip: false,
    adds: Adds::Sound,
    ..SOUND
};

/// A row of one line with nothing of its own.
const PLAIN: Kind = Kind {
    id: "",
    name: |_| String::new(),
    hue: tokens::HUE_CLIP,
    height: 28.0,
    button: Button::None,
    many: None,
    lifts: false,
    apart: false,
    in_clip: true,
    adds: Adds::Nothing,
    ghost: None,
    fill: None,
};

/// A row of sound.
const SOUND: Kind = Kind {
    hue: tokens::HUE_AUDIO,
    height: 36.0,
    ..PLAIN
};

/// A row of the timeline: its kind and, for a kind with tracks, its track (else 0).
#[derive(Clone, Copy)]
pub struct Row {
    pub kind: &'static Kind,
    pub track: u32,
}

impl Row {
    pub const CAPTIONS: Self = Self::of(&CAPTIONS, 0);
    pub const CLIPS: Self = Self::of(&CLIPS, 0);
    pub const ZOOMS: Self = Self::of(&ZOOMS, 0);
    pub const CURSOR: Self = Self::of(&CURSOR, 0);
    pub const KEYS: Self = Self::of(&KEYS, 0);
    pub const MIC: Self = Self::of(&MIC, 0);
    pub const SYSTEM: Self = Self::of(&SYSTEM, 0);

    const fn of(kind: &'static Kind, track: u32) -> Self {
        Self { kind, track }
    }

    pub const fn annotations(track: u32) -> Self {
        Self::of(&ANNOTATIONS, track)
    }

    pub const fn audio(track: u32) -> Self {
        Self::of(&AUDIO, track)
    }

    /// The row of the recordings' sound track `name` (`mic`, else the system sound).
    pub fn channel(name: &str) -> Self {
        match name == dc_capture::audio::MIC_TRACK {
            true => Self::MIC,
            false => Self::SYSTEM,
        }
    }

    /// The row is one of the rows `other` is one of: the rows of a kind with tracks.
    pub fn same_kind(self, other: Self) -> bool {
        self.kind.id == other.kind.id
    }

    /// The track of an annotation row.
    pub fn annotation_track(self) -> Option<u32> {
        self.same_kind(Self::annotations(0)).then_some(self.track)
    }

    /// The track of a row of a kind with tracks.
    pub fn track_of_many(self) -> Option<u32> {
        self.kind.many.as_ref().map(|_| self.track)
    }

    /// The row of the same kind on `track`.
    pub fn on(self, track: u32) -> Self {
        Self::of(self.kind, track)
    }

    pub fn name(self) -> String {
        (self.kind.name)(self.track)
    }
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.same_kind(*other) && self.track == other.track
    }
}

impl Eq for Row {}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind.many {
            Some(_) => write!(f, "{}:{}", self.kind.id, self.track),
            None => f.write_str(self.kind.id),
        }
    }
}
