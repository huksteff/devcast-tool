//! A gesture on the regions while the button is held (mockup 2a, 2b): the moved regions and a
//! dragged edge follow the pointer as ghosts with «start – end» over them, stick to the region
//! edges of every row and to the playhead (`places.rs`, `snap.rs`) with a yellow line while
//! «Привязка» is on, and scroll the view at the lanes' edges; the release asks for the commands
//! (`editor/regions.rs`) as one undo step. A rubber band picks every region it touches.

use std::time::Duration;

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};
use dc_fx_annotations::AnnotId;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{self, ACCENT_HI, BG, SNAP};
use eframe::egui::{Align2, Pos2, Rect, Stroke, StrokeKind, Ui, pos2, vec2};

use super::marks;
use super::paint;
use super::places;
use super::regions::tip;
use super::scale::{TimeScale, timecode};
use super::snap;
use super::tracks::{Region, Row};
use super::viewport::Lanes;
use crate::editor::regions as commands;
use crate::editor::selection::{Item, Selected, Selection, same_moment};
use crate::editor::view::{Gesture, Target};
use crate::editor::{Action, EditorWindow};

/// The ghost is lifted this much and keeps this opacity; the place a region left keeps this
/// share of its colour (as a dragged clip, mockup `.drag`).
const LIFT: f32 = 2.0;
const GHOST: f32 = 0.85;
const LEFT_PLACE: f32 = 0.35;
/// Fill of the rubber band (mockup `.marquee`: .12).
const BAND_FILL: f32 = 0.12;
/// Repaints while a drag presses an edge: the auto-scroll goes on without input.
const EDGE_TICK: Duration = Duration::from_millis(16);

/// The rows of the frame: each with its lane, and the time scale.
struct Frame<'a> {
    rows: &'a [(Row, Rect)],
    scale: &'a TimeScale,
}

/// The picked regions are drawn by the gesture, not with their rings.
pub fn follows(gesture: &Option<Gesture>) -> bool {
    matches!(gesture, Some(Gesture::Move { .. } | Gesture::Edge { .. }))
}

/// Runs the gesture of this frame, if any.
pub fn run(
    ui: &Ui,
    rows: &[(Row, Rect)],
    editor: &mut EditorWindow,
    scale: &TimeScale,
    lanes: &Lanes,
) {
    let Some(gesture) = editor.view.gesture.clone() else {
        return;
    };
    let (down, pointer, dt) = ui.input(|i| {
        let p = &i.pointer;
        (p.primary_down(), p.interact_pos(), i.stable_dt)
    });
    let Some(at) = pointer else { return };
    let frame = Frame { rows, scale };
    if !down {
        editor.view.gesture = None;
    }
    let scrolls = !matches!(gesture, Gesture::Band { .. });
    match gesture {
        Gesture::Band { from, base } => {
            band(ui, &frame, editor, Rect::from_two_pos(from, at), base)
        }
        Gesture::Move { grab, row, item } => {
            moving(ui, &frame, editor, (grab, row, item), at, down)
        }
        Gesture::Edge { target, end } => edge(ui, &frame, editor, target, end, at.x, down),
        Gesture::Key {
            id,
            from,
            grab,
            row,
        } => key(ui, &frame, editor, (id, from, grab, row), at.x, down),
    }
    if down && scrolls && editor.view.timeline.edge_scroll(lanes, at.x, dt) {
        ui.ctx().request_repaint_after(EDGE_TICK);
    }
}

/// The picked regions follow the pointer from `grab` (pressed on row `from`, region `held`).
fn moving(
    ui: &Ui,
    frame: &Frame,
    editor: &mut EditorWindow,
    (grab, from, held): (OutputTime, usize, Item),
    at: Pos2,
    down: bool,
) {
    let group = places::group(editor, frame.rows, frame.scale, (grab, from), at);
    let delta = group.shift.delta;
    if !down {
        if delta != TimeDelta::ZERO || group.rows_by != 0 {
            let moved: Vec<_> = group.regions.iter().map(|(_, _, m, _)| *m).collect();
            let steps = commands::moves(&moved, delta > TimeDelta::ZERO);
            editor.ask(Action::Apply(steps));
        }
        return;
    }
    for (region, row, moved, lands) in &group.regions {
        let lane = frame.rows[*row].1;
        let body = paint::body(lane, frame.scale, region.range);
        let left = BG.gamma_multiply(1.0 - LEFT_PLACE);
        ui.painter_at(lane).rect_filled(body, 4, left);
        let (row, lane) = frame.rows[*lands];
        let ghost = ghost(ui, lane, frame.scale, region, moved.range, row);
        if moved.item == held {
            span_tip(ui, ghost, moved.range);
        }
    }
    snap_line(ui, frame, group.shift.stuck);
}

/// An edge of a region or of a speed band follows the pointer at x.
fn edge(
    ui: &Ui,
    frame: &Frame,
    editor: &mut EditorWindow,
    target: Target,
    end: bool,
    x: f32,
    down: bool,
) {
    let Some(stretch) = places::stretch(editor, frame.scale, target, end, x) else {
        editor.view.gesture = None;
        return;
    };
    let (range, at) = (stretch.range, stretch.edge.at);
    if !down {
        let step = match target {
            _ if stretch.now == range => None,
            Target::Region(item) => {
                let project = editor.doc.project();
                Some(commands::resize(project, item, range, end, at))
            }
            Target::Speed { factor, .. } => commands::speed_edge(range, end, at, factor),
        };
        if let Some(step) = step {
            editor.ask(Action::Apply(vec![step]));
        }
        return;
    }
    let Some(&(row, lane)) = frame.rows.get(stretch.row) else {
        return;
    };
    let body = match target {
        Target::Region(_) => ghost(ui, lane, frame.scale, &stretch.region, stretch.now, row),
        Target::Speed { .. } => {
            let band = paint::speed_band(lane, frame.scale, stretch.now);
            let painter = ui.painter_at(lane);
            let shown = Region {
                range: stretch.now,
                ..stretch.region
            };
            paint::region_body(&painter, band, &shown, tokens::HUE_SPEED);
            paint::selected(&painter, band);
            band
        }
    };
    span_tip(ui, body, stretch.now);
    snap_line(ui, frame, stretch.edge.stuck);
}

/// A key marker of annotation `id` follows the pointer at x inside its annotation from `from`
/// (pressed at time `grab` on row `row`); the release moves the key there and keeps it picked.
fn key(
    ui: &Ui,
    frame: &Frame,
    editor: &mut EditorWindow,
    (id, from, grab, row): (AnnotId, OutputTime, OutputTime, usize),
    x: f32,
    down: bool,
) {
    let item = Some(Item::Annotation(id));
    let region = editor.tracks.rows.get(row).map(|t| &t.regions);
    let range = region
        .and_then(|r| r.iter().find(|r| r.item == item))
        .map(|r| r.range);
    let (Some(range), Some(&(_, lane))) = (range, frame.rows.get(row)) else {
        editor.view.gesture = None;
        return;
    };
    let raw = from + frame.scale.time(x).since(grab);
    let targets = places::targets(editor, &[], None);
    let px = frame.scale.px_per_sec;
    let point = snap::point(raw, &targets, px, (range.start(), range.end()));
    if !down {
        if !same_moment(point.at, from) {
            editor.ask(Action::Apply(vec![commands::key_move(id, from, point.at)]));
            let at = point.at;
            editor.view.selection.only(Selected::Key { id, at });
        }
        return;
    }
    let y = paint::body(lane, frame.scale, range).center().y;
    let centre = pos2(frame.scale.x(point.at), y);
    marks::diamond(&ui.painter_at(lane), centre, true);
    let above = pos2(centre.x, lane.top() - 2.0);
    tip(ui, above, Align2::CENTER_BOTTOM, timecode(point.at), true);
    snap_line(ui, frame, point.stuck);
}

/// `region` drawn at `range` on `lane` of `row`, lifted, with the white ring; its body.
fn ghost(
    ui: &Ui,
    lane: Rect,
    scale: &TimeScale,
    region: &Region,
    range: Range<Output>,
    row: Row,
) -> Rect {
    let body = paint::body(lane, scale, range).translate(vec2(0.0, -LIFT));
    let painter = ui.painter_at(lane.expand2(vec2(0.0, LIFT)));
    let mut lifted = painter.clone();
    lifted.multiply_opacity(GHOST);
    let shown = Region {
        range,
        ..region.clone()
    };
    paint::region_body(&lifted, body, &shown, paint::hue(row));
    paint::selected(&painter, body);
    body
}

/// «0:32.40 – 0:36.80» over `body` (mockup 2b).
fn span_tip(ui: &Ui, body: Rect, range: Range<Output>) {
    let line = text::span(&timecode(range.start()), &timecode(range.end()));
    let corner = pos2(body.left(), body.top() - 4.0);
    tip(ui, corner, Align2::LEFT_BOTTOM, line, true);
}

/// The yellow line through all rows where an edge stuck (mockup `.snapline`: 1.5).
fn snap_line(ui: &Ui, frame: &Frame, stuck: Option<OutputTime>) {
    let (Some(at), Some(first), Some(last)) = (stuck, frame.rows.first(), frame.rows.last()) else {
        return;
    };
    let x = frame.scale.x(at);
    let painter = ui.painter_at(first.1.union(last.1));
    painter.vline(x, first.1.top()..=last.1.bottom(), Stroke::new(1.5, SNAP));
}

/// The rubber band `rect`: the regions it touches and `base` are picked.
fn band(ui: &Ui, frame: &Frame, editor: &mut EditorWindow, rect: Rect, base: Selection) {
    let mut picked = base;
    for (index, &(_, lane)) in frame.rows.iter().enumerate() {
        for region in &editor.tracks.rows[index].regions {
            let Some(item) = region.item else { continue };
            let body = paint::body(lane, frame.scale, region.range).intersect(lane);
            if body.intersects(rect) {
                picked.add(Selected::Region(item));
            }
        }
    }
    editor.view.selection = picked;
    if editor.view.gesture.is_none() {
        return;
    }
    let fill = ACCENT_HI.gamma_multiply(BAND_FILL);
    let stroke = Stroke::new(1.0, ACCENT_HI);
    ui.painter().rect(rect, 0, fill, stroke, StrokeKind::Inside);
}
