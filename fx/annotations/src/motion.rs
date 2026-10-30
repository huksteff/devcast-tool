//! Entrance and exit animations (docs/porting/annotations.md §3): what an annotation looks like
//! while it comes in and goes out. They run on the output video, so a sped-up part does not
//! hurry them and a cut does not skip them (decision 000).

use std::f64::consts::PI;

use dc_core::{OutputTime, Range, SourceTime, TimeDelta, TimeMap};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{Annotation, Kind};

/// Default length of an entrance or exit, output time (v1 `inDurationMs`/`outDurationMs`).
pub const STAGE_DEFAULT: TimeDelta = TimeDelta::from_millis(300);
/// Longest entrance or exit, output time.
pub const STAGE_MAX: TimeDelta = TimeDelta::from_millis(10_000);
/// How far a slide starts, reference px (v1): along x and along y.
const SLIDE: [f64; 2] = [96.0, 72.0];

/// How an annotation comes in or goes out. An exit is its entrance played backwards: a slide
/// leaves to the side it is named after, typewriter erases from the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Animation {
    /// Appears and disappears at once.
    #[default]
    None,
    /// Opacity, linear.
    Fade,
    /// Grows from 0.58 of its size with a slight overshoot, fading in.
    Pop,
    /// Slides in from the left (96 px at 1080), fading in.
    SlideLeft,
    /// Slides in from the right.
    SlideRight,
    /// Slides in from above (72 px at 1080).
    SlideTop,
    /// Slides in from below.
    SlideBottom,
    /// Letters appear one by one, linear; text only.
    Typewriter,
    /// The figure is drawn along its outline, linear; shapes and drawings only.
    Draw,
}

impl Animation {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::Pop => "pop",
            Self::SlideLeft => "slide_left",
            Self::SlideRight => "slide_right",
            Self::SlideTop => "slide_top",
            Self::SlideBottom => "slide_bottom",
            Self::Typewriter => "typewriter",
            Self::Draw => "draw",
        }
    }

    /// Whether an annotation of `kind` can show it: typewriter needs letters, draw an outline.
    pub fn fits(self, kind: Kind) -> bool {
        match self {
            Self::Typewriter => kind == Kind::Text,
            Self::Draw => matches!(kind, Kind::Shape | Kind::Drawing),
            _ => true,
        }
    }

    /// The look `p` of the way in (0 — not yet in, 1 — fully in).
    fn pose(self, p: f64) -> Pose {
        let p = p.clamp(0.0, 1.0);
        let e = p * p * (3.0 - 2.0 * p);
        let slide = |x: f64, y: f64| Pose {
            opacity: e,
            shift: [x * SLIDE[0] * (1.0 - e), y * SLIDE[1] * (1.0 - e)],
            ..Pose::STILL
        };
        match self {
            Self::None => Pose::STILL,
            Self::Fade => Pose {
                opacity: p,
                ..Pose::STILL
            },
            Self::Pop => Pose {
                opacity: e,
                scale: 0.58 + 0.42 * e + 0.14 * (PI * e).sin(),
                ..Pose::STILL
            },
            Self::SlideLeft => slide(-1.0, 0.0),
            Self::SlideRight => slide(1.0, 0.0),
            Self::SlideTop => slide(0.0, -1.0),
            Self::SlideBottom => slide(0.0, 1.0),
            Self::Typewriter | Self::Draw => Pose {
                reveal: p,
                ..Pose::STILL
            },
        }
    }
}

/// An entrance or an exit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Stage {
    pub animation: Animation,
    /// Length, output time (a sped-up part does not shorten it), `0..=STAGE_MAX`. Applied no
    /// longer than the annotation is shown; the entrance is served first.
    pub duration: TimeDelta,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            animation: Animation::None,
            duration: STAGE_DEFAULT,
        }
    }
}

impl Stage {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// What the stages do to an annotation at a moment; the identity outside them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// 0..1, multiplies the annotation's own opacity.
    pub opacity: f64,
    /// Shift of the box, reference px (`REFERENCE`), right and down; not scaled by `scale`.
    pub shift: [f64; 2],
    /// Size around the box centre, with every length of the kind.
    pub scale: f64,
    /// Part shown, 0..1: letters of a typewriter, path length of a draw-in.
    pub reveal: f64,
}

impl Pose {
    pub const STILL: Self = Self {
        opacity: 1.0,
        shift: [0.0, 0.0],
        scale: 1.0,
        reveal: 1.0,
    };

    /// The pose of `annotation` at output moment `t`, which shows its recording at `now`.
    /// Time since the first shown moment and till the end are counted on the output video.
    pub fn at(annotation: &Annotation, map: &TimeMap, now: SourceTime, t: OutputTime) -> Self {
        let output = |from: SourceTime, to: SourceTime| {
            Range::new(from, to)
                .ok()
                .and_then(|r| map.output_span(&annotation.media, r))
        };
        let range = annotation.range;
        let seconds = |d: TimeDelta| d.as_secs_f64();
        let since = seconds(shown_for(annotation, map, now, t));
        let left = output(now, range.end()).map_or(0.0, |s| seconds(s.end().since(t)));
        let length = |stage: &Stage, room: f64| match stage.animation {
            Animation::None => 0.0,
            _ => seconds(stage.duration).min(room).max(0.0),
        };
        let enter = length(&annotation.enter, since + left);
        let exit = length(&annotation.exit, since + left - enter);
        if since < enter {
            annotation.enter.animation.pose(since / enter)
        } else if left < exit {
            annotation.exit.animation.pose(left / exit)
        } else {
            Self::STILL
        }
    }
}

/// Output time since `annotation` first shows, at output moment `t`, which shows its recording
/// at `now`: the clock of entrances and of animated pictures (a sped-up part does not hurry it,
/// a cut does not skip it).
pub fn shown_for(
    annotation: &Annotation,
    map: &TimeMap,
    now: SourceTime,
    t: OutputTime,
) -> TimeDelta {
    Range::new(annotation.range.start(), now)
        .ok()
        .and_then(|r| map.output_span(&annotation.media, r))
        .map_or(TimeDelta::ZERO, |s| t.since(s.start()))
}
