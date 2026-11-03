//! Regions of the effects as registry commands: a moved group, a dragged edge, removal. The
//! commands are the effects' own (`dc_ui_kit::lane::Lane`, found by `lanes::of`): each keeps its
//! own rules (a zoom does not overlap a zoom, a cue stays in its recording); a command that
//! refuses fails the whole gesture, and the editor shows why. The speed bands are the editor's
//! (montage, `speed`).

use dc_core::time::Output;
use dc_core::{OutputTime, Project, Range};
use dc_ops::Step;
pub use dc_ui_kit::lane::Moved;
use dc_ui_kit::lane::Resize;
use serde_json::{Map, Value};

use super::lanes::{self, annotations::AnnotId};
use super::selection::Item;

/// The commands that put each region of `moved` in its new place, one after another so that
/// no step meets a region that has not moved yet: the front of the group goes first (`forward`
/// — the group moved later in time).
pub fn moves(moved: &[Moved], forward: bool) -> Vec<Step> {
    let mut order: Vec<&Moved> = moved.iter().collect();
    order.sort_by_key(|m| m.range.start());
    if forward {
        order.reverse();
    }
    order
        .into_iter()
        .filter_map(|m| Some((lanes::of(m.item)?.moved)(m)))
        .collect()
}

/// The command that drags the `end` (or the start) of `item` lying at `range` to `at`.
pub fn resize(
    project: &Project,
    item: Item,
    range: Range<Output>,
    end: bool,
    at: OutputTime,
) -> Option<Step> {
    let resize = Resize {
        item,
        range,
        end,
        at,
    };
    Some((lanes::of(item)?.resized)(project, &resize))
}

/// `<effect> rm <id>`.
pub fn remove(item: Item) -> Option<Step> {
    Some(lanes::of(item)?.removed(item))
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
pub fn key_remove(id: AnnotId, at: OutputTime) -> Option<Step> {
    let item = Item::of(id);
    Some(lanes::of(item)?.key_removed(item, at))
}

/// `annot key move <id> --at <from> --to <to>`.
pub fn key_move(id: AnnotId, from: OutputTime, to: OutputTime) -> Option<Step> {
    let item = Item::of(id);
    Some(lanes::of(item)?.key_moved(item, from, to))
}
