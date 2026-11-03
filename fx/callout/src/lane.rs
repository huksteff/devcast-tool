//! The callout events on the «Курсор» row (`dc_ui_kit::lane`): a region per event that shows,
//! «вкл», «выкл» or its text; a moved event is `callout set --at`, a dragged edge changes its
//! `--duration` (the start keeps the end), a copy is `callout add` with all of the event, a new
//! one (the ghost) `callout add --action on`.

use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, StateError, Step, TimeDelta, TimeMap};
use dc_ui_kit::lane::{self, Item, Lane, Moved, Region, Resize, id_args, seconds, time};
use dc_ui_kit::strings::timeline as text;

use crate::{Action, Callout, EventKind};

pub const LANE: Lane = Lane {
    prefix: EventKind::PREFIX,
    op: "callout",
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("at".to_owned(), time(moved.range.start()));
    Step::new("callout set", args)
}

fn resized(_: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let (range, at) = (resize.range, resize.at);
    if resize.end {
        args.insert("duration".to_owned(), seconds(at.since(range.start())));
    } else {
        args.insert("at".to_owned(), time(at));
        args.insert("duration".to_owned(), seconds(range.end().since(at)));
    }
    Step::new("callout set", args)
}

fn copied(project: &Project, item: Item, _: TimeDelta) -> Option<Step> {
    let id = item.id::<EventKind>()?;
    let state = project.state::<Callout>().ok()?;
    let event = state.events.into_iter().find(|e| e.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("action".to_owned(), event.action.name().into());
    if let Action::Text { text } = &event.action {
        args.insert("text".to_owned(), text.clone().into());
    }
    args.insert("duration".to_owned(), seconds(event.duration));
    Some(Step::new("callout add", args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    lane::in_place(copied(project, item, range.duration()), range)
}

/// The regions of the events that show on the output.
pub fn regions(project: &Project, map: &TimeMap) -> Result<Vec<Region>, StateError> {
    let state = project.state::<Callout>()?;
    let mut regions = Vec::new();
    for event in &state.events {
        let label = match &event.action {
            Action::On => text::CALLOUT_ON.to_owned(),
            Action::Off => text::CALLOUT_OFF.to_owned(),
            Action::Text { text } => text.clone(),
        };
        regions.extend(event.output(map).map(|range| Region {
            item: Some(Item::of(event.id)),
            ..Region::new(range, label)
        }));
    }
    Ok(regions)
}

/// `callout add --action on` at the start of `range`: the event keeps its own length.
pub fn added(range: Range<Output>) -> Step {
    let mut args = serde_json::Map::new();
    args.insert("at".to_owned(), time(range.start()));
    args.insert("action".to_owned(), serde_json::Value::from("on"));
    Step::new("callout add", args)
}
