//! Where the regions of a gesture go, from the pointer: the moved group (with a track change
//! between rows of annotations or of extra sound) and a dragged edge, snapped by `snap.rs`.

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};
use eframe::egui::{Pos2, Rect};

use super::scale::TimeScale;
use super::snap::{self, Moving};
use super::tracks::{Region, Row};
use crate::editor::EditorWindow;
use crate::editor::regions::Moved;
use crate::editor::selection::Item;
use crate::editor::view::Target;

/// A moved group: each region with where it was (row index, region), where it goes and the row
/// it lands on; the shift and the target it stuck to.
pub struct Move {
    pub regions: Vec<(Region, usize, Moved, usize)>,
    pub shift: snap::Shift,
    /// Rows the group went up or down.
    pub rows_by: i64,
}

/// A dragged edge: the region (or speed band) on row `row` from `range` to `now`.
pub struct Stretch {
    pub row: usize,
    pub region: Region,
    pub range: Range<Output>,
    pub now: Range<Output>,
    pub edge: snap::Edge,
}

/// The picked regions with the pointer at `at`, pressed at time `grab` on row `from`.
pub fn group(
    editor: &EditorWindow,
    rows: &[(Row, Rect)],
    scale: &TimeScale,
    (grab, from): (OutputTime, usize),
    at: Pos2,
) -> Move {
    let items: Vec<Item> = editor.view.selection.regions().collect();
    let placed: Vec<(usize, Region)> = items.iter().filter_map(|&i| located(editor, i)).collect();
    let stills: Vec<Vec<Range<Output>>> = placed
        .iter()
        .map(|(row, _)| still(editor, *row, &items))
        .collect();
    let moving: Vec<Moving> = placed
        .iter()
        .zip(&stills)
        .map(|((_, region), still)| Moving {
            range: region.range,
            still,
        })
        .collect();
    let raw = TimeDelta::from_secs_f64(f64::from((at.x - scale.x(grab)) / scale.px_per_sec));
    let targets = targets(editor, &items, None);
    let shift = snap::shift(&moving, raw, &targets, scale.px_per_sec, editor.end());
    let over = rows
        .iter()
        .position(|(_, lane)| lane.y_range().contains(at.y));
    let (rows_by, kind) = track_shift(rows, from, over, &placed);
    let regions = placed
        .into_iter()
        .filter_map(|(row, region)| {
            let item = region.item?;
            let start = region.range.start() + shift.delta;
            let range = Range::new(start, region.range.end() + shift.delta).ok()?;
            let track = track_of(rows.get(row)?.0)
                .filter(|_| rows_by != 0 && kind == Some(Kind::of(rows[row].0)))
                .map(|t| (i64::from(t) + rows_by) as u32);
            let lands = match (track, rows[row].0) {
                (Some(t), Row::Annotations(_)) => row_index(rows, Row::Annotations(t)),
                (Some(t), _) => row_index(rows, Row::Audio(t)),
                (None, _) => Some(row),
            }?;
            let moved = Moved { item, range, track };
            Some((region, row, moved, lands))
        })
        .collect();
    Move {
        regions,
        shift,
        rows_by,
    }
}

/// The `end` (or start) edge of `target` with the pointer at `x`; `None` when it is gone.
pub fn stretch(
    editor: &EditorWindow,
    scale: &TimeScale,
    target: Target,
    end: bool,
    x: f32,
) -> Option<Stretch> {
    let (row, region, still, targets) = match target {
        Target::Region(item) => {
            let (row, region) = located(editor, item)?;
            let still = still(editor, row, &[item]);
            (row, region, still, targets(editor, &[item], None))
        }
        Target::Speed { range, .. } => {
            let row = editor
                .tracks
                .rows
                .iter()
                .position(|t| t.row == Row::Clips)?;
            let band = editor
                .tracks
                .speeds
                .iter()
                .find(|s| s.region.range == range)?;
            let targets = targets(editor, &[], Some(range));
            (row, band.region.clone(), Vec::new(), targets)
        }
    };
    let range = region.range;
    let raw = scale.time(x);
    let edge = snap::edge(
        range,
        end,
        raw,
        &still,
        &targets,
        scale.px_per_sec,
        editor.end(),
    );
    let now = match end {
        true => Range::new(range.start(), edge.at),
        false => Range::new(edge.at, range.end()),
    }
    .ok()?;
    Some(Stretch {
        row,
        region,
        range,
        now,
        edge,
    })
}

/// Row index and region of `item`.
fn located(editor: &EditorWindow, item: Item) -> Option<(usize, Region)> {
    editor
        .tracks
        .rows
        .iter()
        .enumerate()
        .find_map(|(index, track)| {
            let region = track.regions.iter().find(|r| r.item == Some(item))?;
            Some((index, region.clone()))
        })
}

/// The regions of row `index` that stay and must not be overlapped: zooms and extra sound do
/// not overlap on their row; the others may.
fn still(editor: &EditorWindow, index: usize, moving: &[Item]) -> Vec<Range<Output>> {
    let track = &editor.tracks.rows[index];
    if !matches!(track.row, Row::Zooms | Row::Audio(_)) {
        return Vec::new();
    }
    track
        .regions
        .iter()
        .filter(|r| r.item.is_none_or(|i| !moving.contains(&i)))
        .map(|r| r.range)
        .collect()
}

/// What a dragged edge sticks to: the edges of the regions of all rows and of the speed bands
/// but the dragged ones, and the playhead; nothing with «Привязка» off.
pub fn targets(
    editor: &EditorWindow,
    moving: &[Item],
    band: Option<Range<Output>>,
) -> Vec<OutputTime> {
    if !editor.view.magnet {
        return Vec::new();
    }
    let regions = editor.tracks.rows.iter().flat_map(|t| &t.regions);
    let mut targets: Vec<OutputTime> = regions
        .filter(|r| r.item.is_none_or(|i| !moving.contains(&i)))
        .map(|r| r.range)
        .chain(editor.tracks.speeds.iter().map(|s| s.region.range))
        .filter(|r| Some(*r) != band)
        .flat_map(|r| [r.start(), r.end()])
        .collect();
    targets.push(editor.view.playhead);
    targets
}

/// Regions that change track together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Annotations,
    Sound,
    Other,
}

impl Kind {
    /// The kind of the regions on `row`.
    fn of(row: Row) -> Self {
        match row {
            Row::Annotations(_) => Self::Annotations,
            Row::Audio(_) => Self::Sound,
            _ => Self::Other,
        }
    }
}

/// Track change of a group pressed on row `from` and now over row `over`: between the rows of
/// annotations or of extra sound, kept to the rows there are; the kind it applies to.
fn track_shift(
    rows: &[(Row, Rect)],
    from: usize,
    over: Option<usize>,
    placed: &[(usize, Region)],
) -> (i64, Option<Kind>) {
    let (Some(&(start, _)), Some(&(now, _))) = (rows.get(from), over.and_then(|o| rows.get(o)))
    else {
        return (0, None);
    };
    let (shift, kind) = match (start, now) {
        (Row::Annotations(a), Row::Annotations(b)) => {
            (i64::from(b) - i64::from(a), Kind::Annotations)
        }
        (Row::Audio(a), Row::Audio(b)) => (i64::from(b) - i64::from(a), Kind::Sound),
        _ => return (0, None),
    };
    let count = rows.iter().filter(|(r, _)| Kind::of(*r) == kind).count() as i64;
    let ours = placed
        .iter()
        .filter(|(_, region)| region.item.is_some())
        .filter_map(|(row, _)| rows.get(*row).map(|r| r.0))
        .filter(|row| Kind::of(*row) == kind)
        .filter_map(|row| track_of(row).map(i64::from));
    let (low, high) = ours.fold((i64::MIN, i64::MAX), |(low, high), t| {
        (low.max(-t), high.min(count - 1 - t))
    });
    (shift.clamp(low.min(0), high.max(0)), Some(kind))
}

/// The track of `row`, for the rows that have tracks.
fn track_of(row: Row) -> Option<u32> {
    match row {
        Row::Annotations(t) | Row::Audio(t) => Some(t),
        _ => None,
    }
}

fn row_index(rows: &[(Row, Rect)], row: Row) -> Option<usize> {
    rows.iter().position(|(r, _)| *r == row)
}
