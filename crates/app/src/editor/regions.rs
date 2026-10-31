//! Regions of the effects as registry commands, written as `devcast` takes them (times are
//! seconds with six digits, `montage::arg`): a moved group, a dragged edge, removal. Each
//! effect keeps its own rules (a zoom does not overlap a zoom, a cue stays in its recording); a
//! command that refuses fails the whole gesture, and the editor shows why.

use dc_core::time::Output;
use dc_core::{OutputTime, Project, Range, TimeDelta};
use dc_fx_annotations::AnnotId;
use dc_fx_audio_tracks::Audio;
use dc_ops::Step;
use serde_json::{Map, Value};

use super::montage::arg;
use super::selection::Item;

/// A region after a move: its new place and, for annotations and extra sound, a new track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    pub item: Item,
    pub range: Range<Output>,
    pub track: Option<u32>,
}

/// The command and the id of `item`.
fn command(item: Item, verb: &str) -> (String, Map<String, Value>) {
    let (op, id) = match item {
        Item::Zoom(id) => ("zoom", id.to_string()),
        Item::Cue(id) => ("captions", id.to_string()),
        Item::Callout(id) => ("callout", id.to_string()),
        Item::Annotation(id) => ("annot", id.to_string()),
        Item::Sound(id) => ("audio", id.to_string()),
    };
    let mut args = Map::new();
    args.insert("id".to_owned(), Value::String(id));
    (format!("{op} {verb}"), args)
}

fn time(t: OutputTime) -> Value {
    Value::String(arg(t))
}

/// A length in seconds: the number `devcast` reads from `1.500000`.
pub fn seconds(length: TimeDelta) -> Value {
    Value::from(length.micros() as f64 / 1e6)
}

/// The commands that put each region of `moved` in its new place, one after another so that
/// no step meets a region that has not moved yet: the front of the group goes first (`forward`
/// — the group moved later in time).
pub fn moves(moved: &[Moved], forward: bool) -> Vec<Step> {
    let mut order: Vec<&Moved> = moved.iter().collect();
    order.sort_by_key(|m| m.range.start());
    if forward {
        order.reverse();
    }
    order.into_iter().map(move_step).collect()
}

fn move_step(moved: &Moved) -> Step {
    let (start, end) = (moved.range.start(), moved.range.end());
    let (op, args) = match moved.item {
        Item::Zoom(_) | Item::Cue(_) => {
            let (op, mut args) = command(moved.item, "set");
            args.insert("from".to_owned(), time(start));
            args.insert("to".to_owned(), time(end));
            (op, args)
        }
        Item::Annotation(_) => with_track(command(moved.item, "move"), start, moved.track),
        Item::Callout(_) => with_track(command(moved.item, "set"), start, None),
        Item::Sound(_) => with_track(command(moved.item, "set"), start, moved.track),
    };
    Step::new(op, args)
}

fn with_track(
    (op, mut args): (String, Map<String, Value>),
    at: OutputTime,
    track: Option<u32>,
) -> (String, Map<String, Value>) {
    args.insert("at".to_owned(), time(at));
    if let Some(track) = track {
        args.insert("track".to_owned(), track.into());
    }
    (op, args)
}

/// The command that drags the `end` (or the start) of `item` lying at `range` to `at`. A
/// callout event keeps its end when its start moves; extra sound keeps the sound under its
/// end (the file's start moves too).
pub fn resize(
    project: &Project,
    item: Item,
    range: Range<Output>,
    end: bool,
    at: OutputTime,
) -> Step {
    let (op, mut args) = command(item, "set");
    match (item, end) {
        (Item::Zoom(_) | Item::Cue(_) | Item::Annotation(_), true) => {
            args.insert("to".to_owned(), time(at));
        }
        (Item::Zoom(_) | Item::Cue(_) | Item::Annotation(_), false) => {
            args.insert("from".to_owned(), time(at));
        }
        (Item::Callout(_) | Item::Sound(_), true) => {
            args.insert("duration".to_owned(), seconds(at.since(range.start())));
        }
        (Item::Callout(_) | Item::Sound(_), false) => {
            args.insert("at".to_owned(), time(at));
            args.insert("duration".to_owned(), seconds(range.end().since(at)));
        }
    }
    if let (Item::Sound(id), false) = (item, end) {
        let offset = project
            .state::<Audio>()
            .ok()
            .and_then(|s| s.clips.into_iter().find(|c| c.id == id))
            .map(|c| TimeDelta::from_micros(c.offset.micros() + at.since(range.start()).micros()));
        if let Some(offset) = offset {
            args.insert("offset".to_owned(), seconds(offset));
        }
    }
    Step::new(op, args)
}

/// `<effect> rm <id>`.
pub fn remove(item: Item) -> Step {
    let (op, args) = command(item, "rm");
    Step::new(op, args)
}

/// A speed band from `from` dragged by one edge to `to`: the part it takes gets its
/// `factor`, the part it leaves plays at 1× (`speed` sets the factor of a range of the output
/// as it is now).
pub fn speed_edge(from: Range<Output>, end: bool, to: OutputTime, factor: f64) -> Option<Step> {
    let here = if end { from.end() } else { from.start() };
    let grows = if end { to > here } else { to < here };
    let span = Range::new(here.min(to), here.max(to))
        .ok()
        .filter(|r| !r.is_empty())?;
    let factor = if grows { factor } else { 1.0 };
    let mut args = Map::new();
    args.insert(
        "range".to_owned(),
        Value::String(super::montage::range_arg(span)),
    );
    args.insert("factor".to_owned(), factor.into());
    Some(Step::new("speed", args))
}

/// `annot key rm <id> --at <at>`: the key of annotation `id` at output `at`.
pub fn key_remove(id: AnnotId, at: OutputTime) -> Step {
    let (op, mut args) = command(Item::Annotation(id), "key rm");
    args.insert("at".to_owned(), time(at));
    Step::new(op, args)
}

/// `annot key move <id> --at <from> --to <to>`.
pub fn key_move(id: AnnotId, from: OutputTime, to: OutputTime) -> Step {
    let (op, mut args) = command(Item::Annotation(id), "key move");
    args.insert("at".to_owned(), time(from));
    args.insert("to".to_owned(), time(to));
    Step::new(op, args)
}
