//! The annotations on the timeline (`dc_ui_kit::lane`): a moved annotation is `annot move --at`
//! (with `--track` when it changes row), a dragged edge `annot set --from|--to`, a copy `annot
//! paste` with its JSON. «Дублировать» puts the copy right after the original: on its row when
//! that row is free there, else on the first free row, else on its row on top (as Ctrl+V).

use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, Step, TimeDelta, TimeMap};
use dc_ui_kit::lane::{Item, Lane, Moved, Resize, id_args, seconds, time};

use crate::{AnnotId, AnnotKind, Annotations, TRACKS};

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
