//! The annotations on the timeline (`dc_ui_kit::lane`): a row per track (a higher one is painted
//! above), a region per annotation that shows with the name of its kind and its keys (7.12); a
//! moved annotation is `annot move --at` (with `--track` when it changes row), a dragged edge
//! `annot set --from|--to`, a copy `annot paste` with its JSON, a new one `annot add` of a kind of
//! [`KINDS`] (the ghost's menu; T adds text). «Дублировать» puts the copy right after the
//! original: on its row when that row is free there, else on the first free row, else on its
//! row on top (as Ctrl+V). Rows removed or moved (the row order is the layer order) are
//! `annot rm` and `annot move --track`.

use dc_core::time::Output;
use dc_core::{IdKind, OutputTime, Project, Range, StateError, Step, TimeDelta, TimeMap};
use dc_ui_kit::lane::{Item, Lane, Moved, Region, Resize, id_args, seconds, time};
use dc_ui_kit::strings::timeline as text;

pub use crate::lane_tip::{Fields, Tip, fields, tip};
pub use crate::{AnnotId, Kind, TRACKS, same_moment};
use crate::{AnnotKind, Annotation, Annotations};

pub const LANE: Lane = Lane {
    prefix: AnnotKind::PREFIX,
    op: "annot",
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("at".to_owned(), time(moved.range.start()));
    if let Some(track) = moved.track {
        args.insert("track".to_owned(), track.into());
    }
    Step::new("annot move", args)
}

fn resized(_: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let edge = if resize.end { "to" } else { "from" };
    args.insert(edge.to_owned(), time(resize.at));
    Step::new("annot set", args)
}

fn copied(project: &Project, item: Item, length: TimeDelta) -> Option<Step> {
    let id = item.id::<AnnotKind>()?;
    let state = project.state::<Annotations>().ok()?;
    let annotation = state.annotations.into_iter().find(|a| a.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("item".to_owned(), serde_json::to_value(&annotation).ok()?);
    args.insert("duration".to_owned(), seconds(length));
    Some(Step::new("annot paste", args))
}

/// Past a joint of recordings or the end the command refuses.
fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    let mut step = copied(project, item, range.duration())?;
    let after = Range::new(range.end(), range.end() + range.duration()).ok()?;
    let track = row(project, item.id()?, after)?;
    step.args.insert("at".to_owned(), time(after.start()));
    step.args.insert("track".to_owned(), track.into());
    Some(step)
}

/// The row for a copy of annotation `id` at `place`: its own when free there, else the first
/// free one, else its own.
fn row(project: &Project, id: AnnotId, place: Range<Output>) -> Option<u32> {
    let state = project.state::<Annotations>().ok()?;
    let own = state.annotations.iter().find(|a| a.id == id)?.track;
    let map = TimeMap::build(project);
    let free = |track: u32| {
        state.annotations.iter().all(|a| {
            a.track != track
                || map
                    .output_span(&a.media, a.range)
                    .is_none_or(|shown| shown.intersect(&place).is_none())
        })
    };
    let first = (0..TRACKS).find(|&t| free(t));
    Some(if free(own) { own } else { first.unwrap_or(own) })
}

/// The annotation rows and the keys of all annotations at output times, sorted: the jumps of [
/// and ] (7.5).
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    /// Track by track from the bottom: up to the highest used one, at least the asked count and
    /// one.
    pub rows: Vec<Vec<Region>>,
    pub keys: Vec<OutputTime>,
}

impl Rows {
    /// `least` empty rows (at least one): what a broken state shows.
    pub fn empty(least: u32) -> Self {
        Self {
            rows: vec![Vec::new(); least.clamp(1, TRACKS) as usize],
            keys: Vec::new(),
        }
    }
}

/// The rows of the annotations of `project`, at least `least`.
pub fn rows(project: &Project, map: &TimeMap, least: u32) -> Result<Rows, StateError> {
    let state = project.state::<Annotations>()?;
    let count = state
        .annotations
        .iter()
        .map(|a| a.track + 1)
        .max()
        .unwrap_or(1)
        .max(least.clamp(1, TRACKS));
    let rows = (0..count)
        .map(|track| {
            let on_track = state.annotations.iter().filter(|a| a.track == track);
            let regions = on_track.filter_map(|a| {
                let range = map.output_span(&a.media, a.range)?;
                Some(Region {
                    item: Some(Item::of(a.id)),
                    keys: keys(map, a, range),
                    ..Region::new(range, kind_name(a.kind).to_owned())
                })
            });
            regions.collect()
        })
        .collect();
    let mut all = Vec::new();
    for annotation in &state.annotations {
        if let Some(span) = map.output_span(&annotation.media, annotation.range) {
            all.extend(keys(map, annotation, span));
        }
    }
    all.sort_unstable();
    all.dedup();
    Ok(Rows { rows, keys: all })
}

/// Output times of the keys of `annotation`, which shows at `span`: a key past a trimmed edge or
/// in a cut does not show; a key at the end is at the end of the span.
fn keys(map: &TimeMap, annotation: &Annotation, span: Range<Output>) -> Vec<OutputTime> {
    let range = annotation.range;
    let times = annotation.keys.iter().filter_map(|key| {
        let t = range.start() + key.at;
        if t == range.end() {
            return Some(span.end());
        }
        range.contains(t).then_some(())?;
        map.locate(&annotation.media, t).map(|(_, at)| at)
    });
    times.collect()
}

/// The name of `kind` on its regions and in the menu of a new annotation.
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Text => text::TEXT,
        Kind::Shape => text::SHAPE,
        Kind::Image => text::IMAGE,
        Kind::Drawing => text::DRAWING,
        Kind::Blur => text::BLUR,
    }
}

/// The kinds a new annotation can be, in the order of the menu that adds one.
pub const KINDS: [Kind; 5] = [
    Kind::Text,
    Kind::Shape,
    Kind::Image,
    Kind::Drawing,
    Kind::Blur,
];

/// `annot add` of a `kind` annotation at `range` on `track`.
pub fn added(range: Range<Output>, track: u32, kind: Kind) -> Step {
    let mut args = serde_json::Map::new();
    args.insert("at".to_owned(), time(range.start()));
    args.insert("kind".to_owned(), serde_json::Value::from(kind.name()));
    args.insert("track".to_owned(), track.into());
    args.insert("duration".to_owned(), seconds(range.duration()));
    Step::new("annot add", args)
}

/// The id `annot add` gives the next annotation of `project`.
pub fn next_id(project: &Project) -> Result<AnnotId, StateError> {
    let state = project.state::<Annotations>()?;
    Ok(AnnotId::from_number(state.ids.next_number()))
}

/// Each annotation to the track `to` gives (`None` removes it), in the order of the list:
/// `annot move --track` puts it on top of the new track, so every annotation of a changed track
/// moves and each track keeps its own order.
pub fn retrack(project: &Project, to: impl Fn(u32) -> Option<u32>) -> Vec<Step> {
    let Ok(state) = project.state::<Annotations>() else {
        return Vec::new();
    };
    state
        .annotations
        .iter()
        .filter_map(|a| match to(a.track) {
            None => Some(LANE.removed(Item::of(a.id))),
            Some(track) if track != a.track => {
                let mut args = id_args(Item::of(a.id));
                args.insert("track".to_owned(), track.into());
                Some(Step::new("annot move", args))
            }
            Some(_) => None,
        })
        .collect()
}

/// `annot key copy`: the key of `id` at output `at` to output `to`, into annotation `into`.
pub fn key_copied(id: AnnotId, at: OutputTime, to: OutputTime, into: AnnotId) -> Step {
    let mut args = id_args(Item::of(id));
    args.insert("at".to_owned(), time(at));
    args.insert("to".to_owned(), time(to));
    if into != id {
        args.insert("into".to_owned(), Item::of(into).to_string().into());
    }
    Step::new("annot key copy", args)
}
