//! The edit document: clips on the main track (with their cuts and speed ranges) and the state of
//! every effect. Effect state is stored as JSON under the effect id; the effect crate owns its
//! typed form (see [`crate::effect`]). Saving and loading the folder is `dc-project`'s job.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::geom::PixelSize;
use crate::id::{Id, IdCounter, IdKind};
use crate::time::{Range, SourceTime, TimeDelta};

/// Current version of the project schema; bumped together with a migration in `dc-project`.
pub const SCHEMA_VERSION: u32 = 1;

/// Slowest and fastest allowed speed of a range (SCOPE: 0.25–16×).
pub const SPEED_MIN: f64 = 0.25;
pub const SPEED_MAX: f64 = 16.0;

/// Id kind of clips on the main track (`c1`, `c2`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ClipKind;
impl IdKind for ClipKind {
    const PREFIX: &'static str = "c";
    const NOUN: &'static str = "клипа";
    const PLURAL: &'static str = "клипы";
    const LIST: &'static str = "ls clips";
}
pub type ClipId = Id<ClipKind>;

/// Where a clip's media came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// Recorded by DevCast: has cursor telemetry and separate audio tracks.
    Recording,
    /// An imported video file: no cursor telemetry.
    Imported,
}

/// A media file of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Media {
    /// Path relative to the project folder, with `/` separators, e.g. `media/screen.mp4`.
    pub path: String,
    pub kind: MediaKind,
    /// Full length of the source.
    pub duration: TimeDelta,
    /// Frame size of the source video; crop and output aspect are computed from it.
    pub size: PixelSize,
}

/// A part of a source played faster or slower; pitch is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeedRange {
    pub range: Range<crate::time::Source>,
    /// Playback factor, `SPEED_MIN..=SPEED_MAX`; 2.0 plays twice as fast.
    pub factor: f64,
}

/// A clip on the main track. Everything inside is in the clip's own source time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Clip {
    pub id: ClipId,
    pub media: Media,
    /// The used part of the source (in/out points).
    pub trim: Range<crate::time::Source>,
    /// Removed parts inside `trim`: sorted, not overlapping.
    #[serde(default)]
    pub cuts: Vec<Range<crate::time::Source>>,
    /// Speed changes inside `trim`: sorted, not overlapping. Missing parts play at 1×.
    #[serde(default)]
    pub speeds: Vec<SpeedRange>,
}

/// The whole edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub schema_version: u32,
    /// Main track, in playback order (magnetic: no gaps between clips).
    pub clips: Vec<Clip>,
    pub clip_ids: IdCounter<ClipKind>,
    /// State of each effect, keyed by effect id (`"zoom"`, `"cursor"`, …).
    #[serde(default)]
    pub effects: BTreeMap<String, serde_json::Value>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            clips: Vec::new(),
            clip_ids: IdCounter::default(),
            effects: BTreeMap::new(),
        }
    }
}

impl Project {
    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    /// Every broken invariant, as a human-readable line. Empty means the project is consistent.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema_version != SCHEMA_VERSION {
            problems.push(format!(
                "версия схемы {} вместо {SCHEMA_VERSION}: нужна миграция",
                self.schema_version
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for clip in &self.clips {
            if !seen.insert(clip.id) {
                problems.push(format!("{}: id повторяется", clip.id));
            }
            if clip.id.number() >= self.clip_ids.next_number() {
                problems.push(format!("{}: id не меньше счётчика clip_ids", clip.id));
            }
            problems.extend(
                clip.problems()
                    .into_iter()
                    .map(|p| format!("{}: {p}", clip.id)),
            );
        }
        problems
    }
}

impl Clip {
    fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let source_end = SourceTime::ZERO + self.media.duration;
        if self.trim.start() < SourceTime::ZERO || self.trim.end() > source_end {
            problems.push(format!(
                "trim {}–{} выходит за длину исходника {}",
                self.trim.start(),
                self.trim.end(),
                source_end
            ));
        }
        if self.media.size.width == 0 || self.media.size.height == 0 {
            problems.push(format!(
                "пустой размер кадра {}x{}",
                self.media.size.width, self.media.size.height
            ));
        }
        if self.trim.is_empty() {
            problems.push("пустой trim".to_owned());
        }
        problems.extend(ordered_inside("cuts", self.cuts.iter().copied(), self.trim));
        problems.extend(ordered_inside(
            "speeds",
            self.speeds.iter().map(|s| s.range),
            self.trim,
        ));
        for s in &self.speeds {
            if !(SPEED_MIN..=SPEED_MAX).contains(&s.factor) {
                problems.push(format!("скорость {} вне {SPEED_MIN}–{SPEED_MAX}", s.factor));
            }
        }
        problems
    }
}

/// Checks that `ranges` are non-empty, sorted, not overlapping and inside `bounds`.
fn ordered_inside(
    what: &str,
    ranges: impl Iterator<Item = Range<crate::time::Source>>,
    bounds: Range<crate::time::Source>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut previous_end = bounds.start();
    for r in ranges {
        if r.is_empty() {
            problems.push(format!("{what}: пустой диапазон {}", r.start()));
        }
        if r.start() < previous_end {
            problems.push(format!(
                "{what}: {}–{} пересекается с предыдущим или вне порядка",
                r.start(),
                r.end()
            ));
        }
        if r.end() > bounds.end() {
            problems.push(format!("{what}: {}–{} выходит за trim", r.start(), r.end()));
        }
        previous_end = previous_end.max(r.end());
    }
    problems
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
