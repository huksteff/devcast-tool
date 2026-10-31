//! The frame of the picked zoom on the preview (7.21, docs/porting/zoom.md §3): the part of the
//! project frame the zoom fills the output with, drawn over the whole picture (the preview asks
//! the frame without the camera meanwhile). Dragging it moves the focus — a zoom that followed
//! the cursor then stands on that point (mode «Точка», as v1 and the mockup's hint); dragging a
//! corner sets the scale, the frame keeping the output's proportions and the opposite corner
//! (`dc_fx_zoom::resize`). One `zoom set` per gesture; the preview shows the draft meanwhile.

use dc_core::Point;
use dc_fx_zoom::{Corner, Mode, Zoom, ZoomEffect, ZoomId, frame, reachable, resize};
use dc_ops::Step;
use eframe::egui::{CursorIcon, Pos2, Rect, Response, Ui, pos2};
use serde_json::{Map, json};

use super::geom::{GRAB, Mapping};
use super::{Drag, finish, paint, show_draft};
use crate::editor::EditorWindow;

/// What a press on the zoom frame holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    Move,
    Corner(Corner),
}

/// A drag of the zoom frame: the zoom as pressed, the press (canvas px) and the zoom now.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomDrag {
    pub id: ZoomId,
    grip: Grip,
    from: (f64, Point),
    follows: bool,
    at: Pos2,
    now: (f64, Point),
    /// The preview shows a draft of it.
    pub drafted: bool,
}

const CORNERS: [(Corner, (f32, f32)); 4] = [
    (Corner::TopLeft, (0.0, 0.0)),
    (Corner::TopRight, (1.0, 0.0)),
    (Corner::BottomLeft, (0.0, 1.0)),
    (Corner::BottomRight, (1.0, 1.0)),
];

/// The zoom frame of `id` over the frame mapped by `map`; true when the pointer works on it (a
/// press inside it or on a corner, or its drag).
pub fn ui(
    ui: &Ui,
    editor: &mut EditorWindow,
    response: &Response,
    map: Mapping,
    id: ZoomId,
) -> bool {
    let Some(zoom) = find(editor, id) else {
        return false;
    };
    let dragging = match &editor.view.stage {
        Some(Drag::Zoom(drag)) if drag.id == id => Some(drag.clone()),
        _ => None,
    };
    let (scale, focus) = dragging
        .as_ref()
        .map_or((zoom.scale, zoom.focus), |d| d.now);
    let on_screen = screen(map, scale, focus);
    let hovered = response.hover_pos().and_then(|p| grip(on_screen, p));
    if let Some(grip) = dragging.as_ref().map(|d| d.grip).or(hovered) {
        ui.ctx().set_cursor_icon(cursor(grip));
    }
    let mut ours = dragging.is_some();
    if response.drag_started()
        && let Some(press) = ui.input(|i| i.pointer.press_origin())
        && let Some(grip) = grip(on_screen, press)
    {
        editor.view.stage = Some(Drag::Zoom(ZoomDrag {
            id,
            grip,
            from: (zoom.scale, zoom.focus),
            follows: zoom.mode == Mode::Follow,
            at: map.canvas(press),
            now: (zoom.scale, zoom.focus),
            drafted: false,
        }));
        ours = true;
    }
    if let Some(Drag::Zoom(drag)) = &mut editor.view.stage
        && let Some(pointer) = response.interact_pointer_pos()
        && response.dragged()
    {
        let now = dragged(drag, map, map.canvas(pointer));
        if now != drag.now {
            drag.now = now;
            drag.drafted = true;
            let steps = vec![step(drag)];
            show_draft(ui, editor, &steps);
        }
    }
    if response.drag_stopped()
        && let Some(Drag::Zoom(drag)) = editor.view.stage.clone()
    {
        let changed = drag.now != drag.from;
        finish(editor, drag.drafted, changed.then(|| vec![step(&drag)]));
    }
    let (scale, focus) = match &editor.view.stage {
        Some(Drag::Zoom(drag)) => drag.now,
        _ => (zoom.scale, zoom.focus),
    };
    paint::zoom(ui, map, screen(map, scale, focus), scale);
    ours || (response.clicked() && hovered.is_some())
}

fn find(editor: &EditorWindow, id: ZoomId) -> Option<Zoom> {
    let state = editor.doc.project().state::<ZoomEffect>().ok()?;
    state.zooms.into_iter().find(|z| z.id == id)
}

/// The frame of a zoom with `scale` and `focus` on screen.
fn screen(map: Mapping, scale: f64, focus: Point) -> Rect {
    let f = frame(scale, focus);
    let corner =
        |x: f64, y: f64| map.screen(pos2(x as f32 * map.canvas.x, y as f32 * map.canvas.y));
    Rect::from_min_max(corner(f.x(), f.y()), corner(f.x() + f.w(), f.y() + f.h()))
}

/// What the pointer at `p` holds: a corner, the inside.
fn grip(frame: Rect, p: Pos2) -> Option<Grip> {
    let corner = CORNERS.into_iter().find(|(_, (ax, ay))| {
        let spot = frame.min + frame.size() * eframe::egui::vec2(*ax, *ay);
        spot.distance(p) <= GRAB
    });
    match corner {
        Some((corner, _)) => Some(Grip::Corner(corner)),
        None => frame.contains(p).then_some(Grip::Move),
    }
}

fn cursor(grip: Grip) -> CursorIcon {
    match grip {
        Grip::Move => CursorIcon::Move,
        Grip::Corner(Corner::TopLeft | Corner::BottomRight) => CursorIcon::ResizeNwSe,
        Grip::Corner(Corner::TopRight | Corner::BottomLeft) => CursorIcon::ResizeNeSw,
    }
}

/// Scale and focus with the pointer at `p` (canvas px).
fn dragged(drag: &ZoomDrag, map: Mapping, p: Pos2) -> (f64, Point) {
    let fraction = |p: Pos2| Point {
        x: f64::from(p.x / map.canvas.x),
        y: f64::from(p.y / map.canvas.y),
    };
    let (scale, focus) = drag.from;
    match drag.grip {
        Grip::Move => {
            let (now, then) = (fraction(p), fraction(drag.at));
            let start = reachable(focus, scale);
            let moved = Point {
                x: start.x + now.x - then.x,
                y: start.y + now.y - then.y,
            };
            (scale, round_point(reachable(moved, scale)))
        }
        Grip::Corner(corner) => {
            let (scale, focus) = resize(frame(scale, focus), corner, fraction(p));
            ((scale * 1000.0).round() / 1000.0, round_point(focus))
        }
    }
}

fn round_point(p: Point) -> Point {
    let round = |v: f64| (v * 10_000.0).round() / 10_000.0;
    Point {
        x: round(p.x),
        y: round(p.y),
    }
}

/// `zoom set <id> --scale … --x … --y …`, and `--mode fixed` when a following zoom is moved.
fn step(drag: &ZoomDrag) -> Step {
    let (scale, focus) = drag.now;
    let mut args = Map::new();
    args.insert("id".to_owned(), json!(drag.id));
    args.insert("scale".to_owned(), json!(scale));
    args.insert("x".to_owned(), json!(focus.x));
    args.insert("y".to_owned(), json!(focus.y));
    if drag.follows && drag.grip == Grip::Move {
        args.insert("mode".to_owned(), json!(Mode::Fixed));
    }
    Step::new("zoom set", args)
}
