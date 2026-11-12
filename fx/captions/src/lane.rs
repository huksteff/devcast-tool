//! The cues on the timeline (`dc_ui_kit::lane`): a region per cue that shows, its text, pale
//! while the captions are not shown; a moved cue and a dragged edge are `captions set --from
//! --to` (a cue stays in its recording, the command says why), a copy is `captions paste` with
//! its JSON.

use dc_core::OpName;
use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, StateError, Step, TimeDelta, TimeMap};
use dc_ui_kit::lane::{self, Item, Lane, Moved, Region, Resize, Row, id_args, seconds, time};

use crate::{CaptionsEffect, CueKind};

const CAPTIONS_PASTE: OpName = OpName::of::<crate::CaptionsPaste>();
const CAPTIONS_SET: OpName = OpName::of::<crate::CaptionsSet>();

/// The command of the eye of the row (`--show`).
pub const SHOW: OpName = OpName::of::<crate::CaptionsStyle>();

pub const LANE: Lane = Lane {
    prefix: CueKind::PREFIX,
    rm: OpName::of::<crate::CaptionsRm>(),
    keys: None,
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("from".to_owned(), time(moved.range.start()));
    args.insert("to".to_owned(), time(moved.range.end()));
    Step::of(CAPTIONS_SET, args)
}

fn resized(_: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let edge = if resize.end { "to" } else { "from" };
    args.insert(edge.to_owned(), time(resize.at));
    Step::of(CAPTIONS_SET, args)
}

fn copied(project: &Project, item: Item, length: TimeDelta) -> Option<Step> {
    let id = item.id::<CueKind>()?;
    let state = project.state::<CaptionsEffect>().ok()?;
    let cue = state.cues.into_iter().find(|c| c.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("item".to_owned(), serde_json::to_value(&cue).ok()?);
    args.insert("duration".to_owned(), seconds(length));
    Some(Step::of(CAPTIONS_PASTE, args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    lane::in_place(copied(project, item, range.duration()), range)
}

/// The captions row: the cues that show on the output; off while the captions are not shown.
pub fn row(project: &Project, map: &TimeMap) -> (Row, Option<StateError>) {
    Row::or_empty(read(project, map))
}

fn read(project: &Project, map: &TimeMap) -> Result<Row, StateError> {
    let state = project.state::<CaptionsEffect>()?;
    let off = !state.settings.show;
    let regions = state.cues.iter().filter_map(|cue| {
        let range = map.output_span(&cue.media, cue.range)?;
        Some(Region {
            faded: off,
            item: Some(Item::of(cue.id)),
            ..Region::new(range, cue.text())
        })
    });
    Ok(Row {
        regions: regions.collect(),
        off,
    })
}
