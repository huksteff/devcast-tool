//! The zooms on the timeline (`dc_ui_kit::lane`): a region per zoom that shows, «2×» (follow)
//! or the fixed scale; the commands of a moved region and a dragged edge (`zoom set --from
//! --to`; a zoom does not overlap a zoom, the command says why), a copy (`zoom add` with all of
//! the zoom) and a new zoom (`zoom add`, the ghost shows the default scale).

use dc_core::OpName;
use dc_core::time::Output;
use dc_core::{IdKind, Project, Range, StateError, Step, TimeDelta, TimeMap};
use dc_ui_kit::lane::{self, Item, Lane, Moved, Region, Resize, Row, id_args, seconds, time};
use dc_ui_kit::strings::timeline as text;

use crate::{DEFAULT_SCALE, Mode, ZoomEffect, ZoomKind};

const ZOOM_ADD: OpName = OpName::of::<crate::ZoomAdd>();
const ZOOM_SET: OpName = OpName::of::<crate::ZoomSet>();

pub const LANE: Lane = Lane {
    prefix: ZoomKind::PREFIX,
    rm: OpName::of::<crate::ZoomRm>(),
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
    Step::of(ZOOM_SET, args)
}

fn resized(_: &Project, resize: &Resize) -> Step {
    let mut args = id_args(resize.item);
    let edge = if resize.end { "to" } else { "from" };
    args.insert(edge.to_owned(), time(resize.at));
    Step::of(ZOOM_SET, args)
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
    Some(Step::of(ZOOM_ADD, args))
}

fn duplicated(project: &Project, item: Item, range: Range<Output>) -> Option<Step> {
    lane::in_place(copied(project, item, range.duration()), range)
}

/// The row «Зумы»: the regions of the zooms that show on the output.
pub fn row(project: &Project, map: &TimeMap) -> (Row, Option<StateError>) {
    Row::or_empty(read(project, map))
}

fn read(project: &Project, map: &TimeMap) -> Result<Row, StateError> {
    let state = project.state::<ZoomEffect>()?;
    let regions = state.zooms.iter().filter_map(|zoom| {
        let range = map.output_span(&zoom.media, zoom.range)?;
        let label = match zoom.mode {
            Mode::Follow => text::times(zoom.scale),
            Mode::Fixed => text::fixed_zoom(zoom.scale),
        };
        Some(Region {
            item: Some(Item::of(zoom.id)),
            ..Region::new(range, label)
        })
    });
    Ok(Row {
        regions: regions.collect(),
        off: false,
    })
}

/// `zoom add` of a new zoom at `range`.
pub fn added(range: Range<Output>) -> Step {
    let mut args = serde_json::Map::new();
    args.insert("at".to_owned(), time(range.start()));
    args.insert("duration".to_owned(), seconds(range.duration()));
    Step::of(ZOOM_ADD, args)
}

/// What the ghost of a new zoom says: its scale.
pub fn ghost() -> String {
    text::times(DEFAULT_SCALE)
}
