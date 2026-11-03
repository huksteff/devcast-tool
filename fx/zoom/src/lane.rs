//! The zooms on the timeline (`dc_ui_kit::lane`): the commands of a moved region and a dragged
//! edge (`zoom set --from --to`; a zoom does not overlap a zoom, the command says why), a copy
//! (`zoom add` with all of the zoom).

use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, Step, TimeDelta};
use dc_ui_kit::lane::{self, Item, Lane, Moved, Resize, id_args, seconds, time};

use crate::{ZoomEffect, ZoomKind};

pub const LANE: Lane = Lane {
    prefix: ZoomKind::PREFIX,
    op: "zoom",
    moved,
    resized,
    copied,
    duplicated,
};

fn moved(moved: &Moved) -> Step {
    let mut args = id_args(moved.item);
    args.insert("from".to_owned(), time(moved.range.start()));
    args.insert("to".to_owned(), time(moved.range.end()));
    Step::new("zoom set", args)
}

fn resized(_: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let edge = if resize.end { "to" } else { "from" };
    args.insert(edge.to_owned(), time(resize.at));
    Step::new("zoom set", args)
}

fn copied(project: &Project, item: Item, length: TimeDelta) -> Option<Step> {
    let id = item.id::<ZoomKind>()?;
    let state = project.state::<ZoomEffect>().ok()?;
    let zoom = state.zooms.into_iter().find(|z| z.id == id)?;
    let mut args = serde_json::Map::new();
    args.insert("duration".to_owned(), seconds(length));
    args.insert("scale".to_owned(), zoom.scale.into());
    args.insert("mode".to_owned(), serde_json::to_value(zoom.mode).ok()?);
    args.insert("x".to_owned(), zoom.focus.x.into());
    args.insert("y".to_owned(), zoom.focus.y.into());
    Some(Step::new("zoom add", args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    lane::in_place(copied(project, item, range.duration()), range)
}
