//! Where dragged regions go (mockup 2b, v1 §2.5–2.6): a moved group or a dragged edge sticks to
//! the nearest target within `SNAP` points — the edges of the regions of every row and the
//! playhead — and stays inside the video; on rows whose regions must not overlap (zooms, extra
//! sound) it stops at the neighbours that stay. Pure math over output times.

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};

/// A dragged edge sticks to a target this close, points (v1, mockup 2b: 8).
pub const SNAP: f32 = 8.0;
/// The shortest region a dragged edge leaves, output time (v1: 100 ms; the annotations' and
/// extra sound's least length).
pub const MIN_LENGTH: TimeDelta = TimeDelta::from_millis(100);

/// The moved group: its shift and the target it stuck to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shift {
    pub delta: TimeDelta,
    pub stuck: Option<OutputTime>,
}

/// The dragged edge: where it is and the target it stuck to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub at: OutputTime,
    pub stuck: Option<OutputTime>,
}

/// A region that moves and the regions of its row that stay and must not be overlapped (empty
/// where the row allows overlaps).
#[derive(Debug, Clone, Copy)]
pub struct Moving<'a> {
    pub range: Range<Output>,
    pub still: &'a [Range<Output>],
}

/// The pointer moved `raw` since the press: the shift of `moving`, an edge of which sticks to
/// the nearest of `targets` within `SNAP` at `px_per_sec`, kept inside `0..end` and off the
/// still neighbours. No targets — no sticking («Привязка» off).
pub fn shift(
    moving: &[Moving],
    raw: TimeDelta,
    targets: &[OutputTime],
    px_per_sec: f32,
    end: OutputTime,
) -> Shift {
    let edges = moving.iter().flat_map(|m| [m.range.start(), m.range.end()]);
    let mut best: Option<(f32, TimeDelta, OutputTime)> = None;
    for edge in edges {
        let at = edge + raw;
        for &target in targets {
            let px = distance(at, target, px_per_sec);
            if px <= SNAP && best.is_none_or(|b| px < b.0) {
                best = Some((px, target.since(edge), target));
            }
        }
    }
    let (wanted, stuck) = best.map_or((raw, None), |(_, delta, at)| (delta, Some(at)));
    let (low, high) = shift_limits(moving, end);
    let delta = wanted.max(low).min(high);
    Shift {
        delta,
        stuck: stuck.filter(|_| delta == wanted),
    }
}

/// How far `moving` may shift: back to zero, on to `end`, up to the still neighbours. Always
/// takes in no shift, even for a region that already broke a limit.
pub fn shift_limits(moving: &[Moving], end: OutputTime) -> (TimeDelta, TimeDelta) {
    let mut low = TimeDelta::from_micros(i64::MIN / 2);
    let mut high = TimeDelta::from_micros(i64::MAX / 2);
    for m in moving {
        let (start, stop) = (m.range.start(), m.range.end());
        low = low.max(OutputTime::ZERO.since(start));
        high = high.min(end.since(stop));
        for still in m.still {
            if still.end() <= start {
                low = low.max(still.end().since(start));
            }
            if still.start() >= stop {
                high = high.min(still.start().since(stop));
            }
        }
    }
    (low.min(TimeDelta::ZERO), high.max(TimeDelta::ZERO))
}

/// The edge (`end` or the start) of `range` dragged to `raw`: stuck to the nearest of `targets`
/// within `SNAP`, kept between the still neighbours (or `0..video`) and `MIN_LENGTH` from the
/// other edge.
pub fn edge(
    range: Range<Output>,
    end: bool,
    raw: OutputTime,
    still: &[Range<Output>],
    targets: &[OutputTime],
    px_per_sec: f32,
    video: OutputTime,
) -> Edge {
    let nearest = targets
        .iter()
        .map(|&t| (distance(raw, t, px_per_sec), t))
        .filter(|(px, _)| *px <= SNAP)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, t)| t);
    let wanted = nearest.unwrap_or(raw);
    let (low, high) = edge_limits(range, end, still, video);
    let at = wanted.max(low).min(high);
    Edge {
        at,
        stuck: nearest.filter(|_| at == wanted),
    }
}

/// Where the edge of `range` may go: see [`edge`]. The current place is always inside.
pub fn edge_limits(
    range: Range<Output>,
    end: bool,
    still: &[Range<Output>],
    video: OutputTime,
) -> (OutputTime, OutputTime) {
    let (start, stop) = (range.start(), range.end());
    let (low, high) = if end {
        let next = still
            .iter()
            .map(|r| r.start())
            .filter(|&s| s >= stop)
            .fold(video.max(stop), OutputTime::min);
        (start + MIN_LENGTH, next)
    } else {
        let previous = still
            .iter()
            .map(|r| r.end())
            .filter(|&e| e <= start)
            .fold(OutputTime::ZERO.min(start), OutputTime::max);
        (previous, stop - MIN_LENGTH)
    };
    let here = if end { stop } else { start };
    (low.min(here), high.max(here))
}

fn distance(a: OutputTime, b: OutputTime, px_per_sec: f32) -> f32 {
    (a.since(b).as_secs_f64().abs() as f32) * px_per_sec
}
