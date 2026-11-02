//! The brush on the preview (7.24, mockup 3c «Рисунок», decision 000 «Кисть»): over the picked
//! drawing a left drag inside its box draws a pencil stroke; Shift at the press makes it an
//! eraser; Alt held makes the stroke a straight line from its first point (letting go of Alt goes
//! on adding points, as Shift did in v1). A right drag moves the box (`annot.rs`). The circle
//! under the pointer is the stroke's thickness as the frame draws it, on screen. One `drawing add`
//! per stroke, to the key at the playhead as every annotation edit (`--at`); the preview shows
//! the draft meanwhile.

use dc_fx_annotations::{Annotations, key_offset};
use dc_fx_drawing::{DrawingBody, ERASER_WIDTH, StrokeTool};
use dc_ops::Step;
use dc_render::{REFERENCE, Rgba};
use dc_ui_kit::tokens::WHITE;
use eframe::egui::{Color32, CursorIcon, Pos2, Stroke, Ui};
use serde_json::{Value, json};

use super::annot::OnFrame;
use super::geom::{Boxed, Mapping};
use super::{Drag, finish, show_draft};
use crate::editor::EditorWindow;
use crate::editor::inspector::Edit;

/// Thinnest stroke on the canvas, px (as `dc_fx_drawing::evaluate`).
const MIN_PX: f32 = 1.0;

/// A stroke being drawn: its tool, colour and thickness, the box it lies in and its points so
/// far in fractions of the box.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushDrag {
    pub edit: Edit,
    tool: StrokeTool,
    color: Rgba,
    /// px when the short side of the frame is 1080 px.
    width: f64,
    boxed: Boxed,
    points: Vec<[f64; 2]>,
    /// The preview shows a draft of it.
    pub drafted: bool,
}

/// Diameter on screen, points, of a stroke `width` px at 1080p: the canvas draws it
/// `width · k` px (not below 1), the preview shows a canvas px as `map.k` points.
pub fn diameter(width: f64, map: Mapping) -> f32 {
    let k = f64::from(map.canvas.x.min(map.canvas.y)) / REFERENCE;
    ((width * k) as f32).max(MIN_PX) * map.k
}

/// Tool, colour and thickness of a stroke of drawing `on` begun now, `erase` with Shift.
fn brush(editor: &EditorWindow, on: &OnFrame, erase: bool) -> Option<(StrokeTool, Rgba, f64)> {
    let project = editor.doc.project();
    let state = project.state::<Annotations>().ok()?;
    let annotation = state.annotations.iter().find(|a| a.id == on.edit.id)?;
    let offset = key_offset(project, annotation, editor.view.playhead).ok()?;
    let body = annotation.body_at(offset);
    let body: DrawingBody = serde_json::from_value(Value::Object(body)).ok()?;
    let (color, width) = body.pencil();
    Some(match erase {
        true => (StrokeTool::Eraser, color, ERASER_WIDTH),
        false => (StrokeTool::Pencil, color, width),
    })
}

/// The pointer over the picked drawing `on`: the brush circle, the eraser's with Shift.
pub fn hover(ui: &Ui, editor: &EditorWindow, on: &OnFrame, map: Mapping, at: Pos2) {
    let held = match &editor.view.stage {
        Some(Drag::Brush(drag)) => Some(drag.width),
        _ => None,
    };
    let erase = ui.input(|i| i.modifiers.shift);
    let Some(width) = held.or_else(|| brush(editor, on, erase).map(|(_, _, w)| w)) else {
        return;
    };
    ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    let radius = diameter(width, map) / 2.0;
    let painter = ui.painter();
    painter.circle_stroke(at, radius + 1.0, Stroke::new(1.0, Color32::BLACK));
    painter.circle_stroke(at, radius, Stroke::new(1.0, WHITE));
}

/// A left drag begins at `press` (screen) inside the picked drawing `on`.
pub fn start(ui: &Ui, editor: &mut EditorWindow, on: &OnFrame, map: Mapping, press: Pos2) {
    let erase = ui.input(|i| i.modifiers.shift);
    let Some((tool, color, width)) = brush(editor, on, erase) else {
        return;
    };
    editor.view.stage = Some(Drag::Brush(BrushDrag {
        edit: on.edit,
        tool,
        color,
        width,
        boxed: on.boxed,
        points: vec![fraction(&on.boxed, map.canvas(press))],
        drafted: false,
    }));
}

/// The pointer at `p` (canvas px) during the stroke: a point more, or with Alt the line from
/// the first point.
pub fn drag(ui: &Ui, editor: &mut EditorWindow, p: Pos2) {
    let line = ui.input(|i| i.modifiers.alt);
    let Some(Drag::Brush(drag)) = &mut editor.view.stage else {
        return;
    };
    let point = fraction(&drag.boxed, p);
    let mut points = drag.points.clone();
    if line {
        points.truncate(1);
    }
    if points.last() != Some(&point) {
        points.push(point);
    }
    if points == drag.points {
        return;
    }
    drag.points = points;
    drag.drafted = true;
    let steps = vec![step(drag)];
    show_draft(ui, editor, &steps);
}

/// The release: the stroke as one undo step.
pub fn stop(editor: &mut EditorWindow) {
    if let Some(Drag::Brush(drag)) = editor.view.stage.clone() {
        finish(editor, drag.drafted, Some(vec![step(&drag)]));
    }
}

/// `p` (canvas px) in fractions of `boxed` along its axes, kept inside it, to 4 decimals.
fn fraction(boxed: &Boxed, p: Pos2) -> [f64; 2] {
    let v = boxed.local(p) / boxed.size;
    let part = |v: f32| (f64::from(v + 0.5).clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0;
    [part(v.x), part(v.y)]
}

/// `drawing add <id> --points … --width … (--color … | --tool eraser) [--at <key>]`.
fn step(drag: &BrushDrag) -> Step {
    let points: Vec<String> = drag.points.iter().flatten().map(f64::to_string).collect();
    let paint = match drag.tool {
        StrokeTool::Pencil => ("color", json!(drag.color)),
        StrokeTool::Eraser => ("tool", json!(drag.tool.name())),
    };
    drag.edit.step(
        "drawing add",
        [
            ("points", json!(points.join(","))),
            ("width", json!(drag.width)),
            paint,
        ],
    )
}
