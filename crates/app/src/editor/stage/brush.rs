//! The brush on the preview (7.24, mockup 3c «Рисунок», decision 000 «Кисть»): over the picked
//! drawing a left drag inside its box draws a stroke of the tool chosen on the brush bar
//! (`View::brush`, the pencil by default); Shift at the press makes it an eraser, as wide as the
//! bar's size (`View::eraser`); Alt held makes the stroke a straight line from its first point (letting go of Alt goes
//! on adding points, as Shift did in v1). A right drag moves the box (`annot.rs`). The circle
//! under the pointer is the stroke's thickness as the frame draws it, on screen. One `drawing add`
//! per stroke, to the key at the playhead as every annotation edit (`--at`); the preview shows
//! the draft meanwhile.

use dc_fx_annotations::{Annotations, Kind, key_offset};
use dc_fx_drawing::{DrawingBody, StrokeTool, WIDTH_MAX, WIDTH_MIN};
use dc_ops::Step;
use dc_render::{REFERENCE, Rgba};
use dc_ui_kit::strings::annotations as text;
use dc_ui_kit::tokens::{BORDER, FS_S, R2, S2, SURFACE, TEXT_2, WHITE};
use dc_ui_kit::widgets::{Slider, segmented};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, Frame, Margin, Pos2, Rect, RichText, Stroke, Ui, vec2,
};
use serde_json::{Value, json};

use super::annot::{OnFrame, picked};
use super::geom::{Boxed, Mapping};
use super::{Drag, finish, show_draft};
use crate::editor::EditorWindow;
use dc_fx_annotations::panel::Edit;

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

/// Tool, colour and thickness of a stroke of drawing `on` begun now, Shift held or not.
fn brush(editor: &EditorWindow, on: &OnFrame, shift: bool) -> Option<(StrokeTool, Rgba, f64)> {
    let project = editor.doc.project();
    let state = project.state::<Annotations>().ok()?;
    let annotation = state.annotations.iter().find(|a| a.id == on.edit.id)?;
    let offset = key_offset(project, annotation, editor.view.playhead).ok()?;
    let body = annotation.body_at(offset);
    let body: DrawingBody = serde_json::from_value(Value::Object(body)).ok()?;
    let (color, width) = body.pencil();
    Some(match shift || editor.view.brush == StrokeTool::Eraser {
        true => (StrokeTool::Eraser, color, editor.view.eraser),
        false => (StrokeTool::Pencil, color, width),
    })
}

/// The pointer over the picked drawing `on`: the brush circle, the eraser's with Shift.
pub fn hover(ui: &Ui, editor: &EditorWindow, on: &OnFrame, map: Mapping, at: Pos2) {
    let held = match &editor.view.stage {
        Some(Drag::Brush(drag)) => Some(drag.width),
        _ => None,
    };
    let shift = ui.input(|i| i.modifiers.shift);
    let Some(width) = held.or_else(|| brush(editor, on, shift).map(|(_, _, w)| w)) else {
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
    let shift = ui.input(|i| i.modifiers.shift);
    let Some((tool, color, width)) = brush(editor, on, shift) else {
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

/// The brush bar over the top of the preview `area` while a drawing is picked alone: «Карандаш |
/// Ластик» chooses the tool of a left drag, the slider the eraser's thickness. It lies above the
/// frame, so a press on it never starts a stroke.
pub fn bar(ui: &Ui, editor: &mut EditorWindow, area: Rect) {
    if !drawing_picked(editor) {
        return;
    }
    let frame = Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(R2)
        .inner_margin(Margin::symmetric(S2 as i8, (S2 / 2.0) as i8));
    egui::Area::new(ui.scope_id().with("brush-bar"))
        .fixed_pos(area.center_top() + vec2(0.0, S2))
        .pivot(Align2::CENTER_TOP)
        .show(ui.ctx(), |ui| {
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = S2;
                    let view = &mut editor.view;
                    segmented(
                        ui,
                        &mut view.brush,
                        &[
                            (StrokeTool::Pencil, text::BRUSH_PENCIL, true),
                            (StrokeTool::Eraser, text::BRUSH_ERASER, true),
                        ],
                    );
                    ui.label(RichText::new(text::ERASER_SIZE).size(FS_S).color(TEXT_2));
                    Slider::new(&mut view.eraser, WIDTH_MIN..=WIDTH_MAX, text::ERASER_SIZE)
                        .width(96.0)
                        .suffix(text::PX)
                        .show(ui);
                });
            });
        });
}

/// A drawing is picked alone.
fn drawing_picked(editor: &EditorWindow) -> bool {
    let Some(id) = picked(editor) else {
        return false;
    };
    let project = editor.doc.project();
    project.state::<Annotations>().is_ok_and(|state| {
        state
            .annotations
            .iter()
            .any(|a| a.id == id && a.kind == Kind::Drawing)
    })
}
