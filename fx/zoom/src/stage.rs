//! The frame of the picked zoom on the preview (7.21, `dc_ui_kit::stage`, docs/porting/zoom.md
//! §3): the part of the project frame the zoom fills the output with, drawn over the whole picture
//! (the editor asks the frame without the camera meanwhile), the rest dimmed, the scale in the
//! corner. Dragging it moves the focus — a zoom that followed the cursor then stands on that
//! point (mode «Точка», as v1 and the mockup's hint); dragging a corner sets the scale, the frame
//! keeping the output's proportions and the opposite corner ([`resize`]). One `zoom set` per
//! gesture; the preview shows the draft meanwhile.

use dc_core::OpName;
use dc_core::{Point, Project, Step};
use dc_ui_kit::egui::{Color32, CursorIcon, FontId, Pos2, Rect, Response, Stroke, StrokeKind, Ui};
use dc_ui_kit::egui::{pos2, vec2};
use dc_ui_kit::stage::geom::{GRAB, Mapping};
use dc_ui_kit::stage::paint::{HANDLE, canvas, handle};
use dc_ui_kit::stage::{Answer, Cx, Gesture, Held, Picked, Stage};
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{FS_S, HUE_ZOOM, WHITE};
use serde_json::{Map, json};

pub use crate::ZoomId;
use crate::{Corner, Mode, Zoom, ZoomEffect, frame, reachable, resize};

const ZOOM_SET: OpName = OpName::of::<crate::ZoomSet>();

/// Outside the zoom frame: black 45 %.
const DIM: Color32 = Color32::from_black_alpha(115);

/// What a press on the zoom frame holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grip {
    Move,
    Corner(Corner),
}

/// A drag of the zoom frame: the zoom as pressed, the press (canvas px) and the zoom now.
#[derive(Debug, Clone, PartialEq)]
pub struct Drag {
    id: ZoomId,
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

/// The entry of the editor's table of preview gestures: the frame of the zoom picked alone, over
/// the picture without the camera.
pub struct ZoomFrame;

impl Gesture for Drag {
    fn drafted(&self) -> bool {
        self.drafted
    }
}

impl Stage for ZoomFrame {
    fn bare(&self, picked: Option<Picked>) -> bool {
        picked_zoom(picked).is_some()
    }

    /// `ours` when the pointer works on the frame (a press inside it or on a corner, or its
    /// drag).
    fn ui(&self, ui: &Ui, cx: &mut Cx<'_>, response: &Response, map: Mapping) -> Answer {
        let mut answer = Answer::default();
        let Some(id) = picked_zoom(cx.picked) else {
            return answer;
        };
        let Some(zoom) = find(cx.project, id) else {
            return answer;
        };
        let dragging = mine(cx).filter(|d| d.id == id).cloned();
        let (scale, focus) = dragging
            .as_ref()
            .map_or((zoom.scale, zoom.focus), |d| d.now);
        let on_screen = screen(map, scale, focus);
        let hovered = response.hover_pos().and_then(|p| grip(on_screen, p));
        if let Some(grip) = dragging.as_ref().map(|d| d.grip).or(hovered) {
            ui.ctx().set_cursor_icon(cursor(grip));
        }
        answer.ours = dragging.is_some();
        if response.drag_started()
            && let Some(press) = ui.input(|i| i.pointer.press_origin())
            && let Some(grip) = grip(on_screen, press)
        {
            *cx.held = Some(Held::new(Drag {
                id,
                grip,
                from: (zoom.scale, zoom.focus),
                follows: zoom.mode == Mode::Follow,
                at: map.canvas(press),
                now: (zoom.scale, zoom.focus),
                drafted: false,
            }));
            answer.ours = true;
        }
        if let Some(drag) = cx.held.as_mut().and_then(Held::get_mut::<Drag>)
            && let Some(pointer) = response.interact_pointer_pos()
            && response.dragged()
        {
            let now = dragged(drag, map, map.canvas(pointer));
            if now != drag.now {
                drag.now = now;
                drag.drafted = true;
                answer.draft = Some(vec![step(drag)]);
            }
        }
        if response.drag_stopped()
            && let Some(ended) = mine(cx)
        {
            let changed = ended.now != ended.from;
            answer.done = Some(changed.then(|| vec![step(ended)]));
        }
        let (scale, focus) = match (&answer.done, mine(cx)) {
            (None, Some(drag)) => drag.now,
            _ => (zoom.scale, zoom.focus),
        };
        paint(ui, map, screen(map, scale, focus), scale);
        answer.ours |= response.clicked() && hovered.is_some();
        answer
    }
}

/// The zoom picked alone (as a region).
fn picked_zoom(picked: Option<Picked>) -> Option<ZoomId> {
    match picked? {
        Picked::Region(item) => item.id(),
        Picked::Key(_) => None,
    }
}

/// The frame drag going on.
fn mine<'c>(cx: &'c Cx<'_>) -> Option<&'c Drag> {
    cx.held.as_ref().and_then(Held::get::<Drag>)
}

fn find(project: &Project, id: ZoomId) -> Option<Zoom> {
    let state = project.state::<ZoomEffect>().ok()?;
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
        let spot = frame.min + frame.size() * vec2(*ax, *ay);
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
fn dragged(drag: &Drag, map: Mapping, p: Pos2) -> (f64, Point) {
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
fn step(drag: &Drag) -> Step {
    let (scale, focus) = drag.now;
    let mut args = Map::new();
    args.insert("id".to_owned(), json!(drag.id));
    args.insert("scale".to_owned(), json!(scale));
    args.insert("x".to_owned(), json!(focus.x));
    args.insert("y".to_owned(), json!(focus.y));
    if drag.follows && drag.grip == Grip::Move {
        args.insert("mode".to_owned(), json!(Mode::Fixed));
    }
    Step::of(ZOOM_SET, args)
}

/// The zoom frame `frame` (screen) of a zoom with `scale`.
fn paint(ui: &Ui, map: Mapping, frame: Rect, scale: f64) {
    let painter = ui.painter();
    let whole = canvas(map);
    let outside = [
        Rect::from_min_max(whole.min, pos2(whole.right(), frame.top())),
        Rect::from_min_max(pos2(whole.left(), frame.bottom()), whole.max),
        Rect::from_min_max(pos2(whole.left(), frame.top()), frame.left_bottom()),
        Rect::from_min_max(frame.right_top(), pos2(whole.right(), frame.bottom())),
    ];
    for part in outside.into_iter().filter(|r| r.is_positive()) {
        painter.rect_filled(part, 0, DIM);
    }
    painter.rect_stroke(
        frame,
        0,
        Stroke::new(2.0, HUE_ZOOM.line),
        StrokeKind::Middle,
    );
    for corner in [
        frame.left_top(),
        frame.right_top(),
        frame.left_bottom(),
        frame.right_bottom(),
    ] {
        handle(ui, corner, HUE_ZOOM.line);
    }
    let label = text::zoom_scale(scale);
    let galley = painter.layout_no_wrap(label, FontId::proportional(FS_S), WHITE);
    let pad = vec2(6.0, 3.0);
    let at = frame.min + vec2(HANDLE, HANDLE);
    let plate = Rect::from_min_size(at, galley.size() + 2.0 * pad);
    painter.rect_filled(plate, 4, HUE_ZOOM.fill);
    painter.galley(plate.min + pad, galley, WHITE);
}
