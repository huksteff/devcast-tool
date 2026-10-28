//! How much a press asks for a zoom (docs/porting/zoom.md §10.2): what the pointer did after it
//! tells a double click, a text selection, an opened dropdown or a text field from a plain
//! click. Within a cluster the strongest press gives the focus. Distances are fractions of the
//! recorded frame, as in v1.

use dc_core::TimeDelta;

use crate::suggest::{Action, Pointer};

/// Strength of each kind of press.
const DOUBLE_CLICK: u32 = 1500;
const TEXT_SELECTION: u32 = 1300;
const DROPDOWN: u32 = 1200;
const TEXT_FIELD: u32 = 1100;
const CLICK: u32 = 900;

/// A release this soon after the press ends a drag.
const RELEASE_WITHIN: TimeDelta = TimeDelta::from_millis(3000);
/// A drag at least this long can select text.
const DRAG_MIN: TimeDelta = TimeDelta::from_millis(200);
/// Moves in `(press + MOVES_FROM, press + MOVES_TO]` show what the press opened.
const MOVES_FROM: TimeDelta = TimeDelta::from_millis(100);
const MOVES_TO: TimeDelta = TimeDelta::from_millis(2000);
/// Fewer moves than this: the pointer stayed, as after a click into a text field.
const MOVES_MIN: usize = 3;
/// The pointer stayed within this distance of the press.
const STILL: f64 = 0.02;
/// A move along an axis longer than this counts.
const TRAVEL: f64 = 0.03;
/// How much one axis must dominate the other: horizontal for a selection, vertical for a dropdown.
const HORIZONTAL: f64 = 1.8;
const VERTICAL: f64 = 1.5;

/// Strength of the press `pointer[index]`; `pointer` is one recording in time order.
pub(crate) fn strength(pointer: &[Pointer], index: usize) -> u32 {
    let press = &pointer[index];
    if matches!(press.action, Action::Press { double: true }) {
        return DOUBLE_CLICK;
    }
    let after = &pointer[index + 1..];
    let release = after
        .iter()
        .take_while(|p| p.at.since(press.at) < RELEASE_WITHIN)
        .find(|p| p.action == Action::Release && p.at > press.at);
    if let Some(release) = release {
        let (dx, dy) = (
            release.point.x - press.point.x,
            release.point.y - press.point.y,
        );
        if release.at.since(press.at) >= DRAG_MIN
            && dx.abs() > TRAVEL
            && dx.abs() > dy.abs() * HORIZONTAL
        {
            return TEXT_SELECTION;
        }
    }
    let moves: Vec<&Pointer> = after
        .iter()
        .take_while(|p| p.at.since(press.at) <= MOVES_TO)
        .filter(|p| p.action == Action::Move && p.at.since(press.at) > MOVES_FROM)
        .collect();
    let far = |p: &&Pointer| (p.point.x - press.point.x).hypot(p.point.y - press.point.y) >= STILL;
    if moves.len() < MOVES_MIN || !moves.iter().any(far) {
        return TEXT_FIELD;
    }
    let (mut sum_x, mut sum_y, mut last) = (0.0, 0.0, press.point);
    for p in &moves {
        sum_x += (p.point.x - last.x).abs();
        sum_y += (p.point.y - last.y).abs();
        last = p.point;
    }
    if last.y - press.point.y > TRAVEL && sum_y > sum_x * VERTICAL {
        return DROPDOWN;
    }
    if sum_x > TRAVEL && sum_x > sum_y * HORIZONTAL {
        return TEXT_SELECTION;
    }
    CLICK
}
