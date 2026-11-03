//! The cues on the timeline (`dc_ui_kit::lane`): a moved cue and a dragged edge are `captions
//! set --from --to` (a cue stays in its recording, the command says why), a copy is `captions
//! paste` with its JSON.

use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, Step, TimeDelta};
use dc_ui_kit::lane::{self, Item, Lane, Moved, Resize, id_args, seconds, time};

use crate::{CaptionsEffect, CueKind};

pub const LANE: Lane = Lane {
    prefix: CueKind::PREFIX,
    op: "captions",
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("from".to_owned(), time(moved.range.start()));
    args.insert("to".to_owned(), time(moved.range.end()));
    Step::new("captions set", args)
}

fn resized(_: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let edge = if resize.end { "to" } else { "from" };
    args.insert(edge.to_owned(), time(resize.at));
    Step::new("captions set", args)
}

fn copied(project: &Project, item: Item, length: TimeDelta) -> Option<Step> {
    let id = item.id::<CueKind>()?;
    let state = project.state::<CaptionsEffect>().ok()?;
    let cue = state.cues.into_iter().find(|c| c.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("item".to_owned(), serde_json::to_value(&cue).ok()?);
    args.insert("duration".to_owned(), seconds(length));
    Some(Step::new("captions paste", args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    lane::in_place(copied(project, item, range.duration()), range)
}
