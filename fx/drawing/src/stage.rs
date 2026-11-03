//! The brush on the preview (7.24, `dc_ui_kit::stage`, mockup 3c «Рисунок», decision 000
//! «Кисть»): over the picked drawing a left drag inside its box draws a stroke of the tool chosen
//! on the brush bar ([`bar`], the pencil by default); Shift at the press makes it an eraser, as
//! wide as the bar's size; Alt held makes the stroke a straight line from its first point
//! (letting go of Alt goes on adding points, as Shift did in v1). The circle under the pointer is
//! the stroke's thickness as the frame draws it, on screen ([`diameter`]). One `drawing add` per
//! stroke, to the key the panel would edit (`--at`); the preview shows the draft meanwhile.
//! Which gesture runs and where the stroke starts are the editor's.

use dc_core::{OutputTime, Project, Step};
use dc_fx_annotations::stage::{Edit, OnFrame};
use dc_fx_annotations::{AnnotId, Annotations, Kind, key_offset};
use dc_render::{REFERENCE, Rgba};
use dc_ui_kit::egui::{
    self, Align2, Color32, CursorIcon, Frame, Margin, Pos2, Rect, RichText, Stroke, Ui, vec2,
};
use dc_ui_kit::stage::geom::{Boxed, Mapping};
use dc_ui_kit::strings::annotations as text;
use dc_ui_kit::tokens::{BORDER, FS_S, R2, S2, SURFACE, TEXT_2, WHITE};
use dc_ui_kit::widgets::{Slider, segmented};
use serde_json::{Value, json};

use crate::{DrawingBody, WIDTH_MAX, WIDTH_MIN};
pub use crate::{ERASER_WIDTH, StrokeTool};

/// Tool, colour and thickness of a stroke about to begin.
pub type Picked = (StrokeTool, Rgba, f64);

/// Thinnest stroke on the canvas, px (as `evaluate`).
const MIN_PX: f32 = 1.0;

/// What the brush bar chose: the tool of a left drag and the eraser's thickness, px at 1080p
/// (`WIDTH_MIN..=WIDTH_MAX`). The editor keeps it (not the project).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brush {
    pub tool: StrokeTool,
    pub eraser: f64,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            tool: StrokeTool::Pencil,
            eraser: ERASER_WIDTH,
        }
    }
}

/// A stroke being drawn: its tool, colour and thickness, the box it lies in and its points so
/// far in fractions of the box.
#[derive(Debug, Clone, PartialEq)]
pub struct Brushing {
    pub edit: Edit,
    tool: StrokeTool,
    color: Rgba,
    /// px when the short side of the frame is 1080 px.
    pub width: f64,
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

/// Tool, colour and thickness of a stroke of drawing `on` begun at `playhead`, Shift held or
/// not.
pub fn pick(
    project: &Project,
    (on, playhead): (&OnFrame, OutputTime),
    brush: Brush,
    shift: bool,
) -> Option<Picked> {
    let state = project.state::<Annotations>().ok()?;
    let annotation = state.annotations.iter().find(|a| a.id == on.edit.id)?;
    let offset = key_offset(project, annotation, playhead).ok()?;
    let body = annotation.body_at(offset);
    let body: DrawingBody = serde_json::from_value(Value::Object(body)).ok()?;
    let (color, width) = body.pencil();
    Some(match shift || brush.tool == StrokeTool::Eraser {
        true => (StrokeTool::Eraser, color, brush.eraser),
        false => (StrokeTool::Pencil, color, width),
    })
}

/// The pointer at `at` (screen) over the picked drawing: the brush circle of a stroke `width`.
pub fn hover(ui: &Ui, map: Mapping, at: Pos2, width: f64) {
    ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    let radius = diameter(width, map) / 2.0;
    let painter = ui.painter();
    painter.circle_stroke(at, radius + 1.0, Stroke::new(1.0, Color32::BLACK));
    painter.circle_stroke(at, radius, Stroke::new(1.0, WHITE));
}

impl Brushing {
    /// A stroke `(tool, color, width)` of drawing `on` begun at `press` (canvas px).
    pub fn new(on: &OnFrame, (tool, color, width): Picked, press: Pos2) -> Self {
        Self {
            edit: on.edit,
            tool,
            color,
            width,
            boxed: on.boxed,
            points: vec![fraction(&on.boxed, press)],
            drafted: false,
        }
    }

    /// The pointer at `p` (canvas px): a point more, or with `line` (Alt) the line from the
    /// first point. True when the stroke changed.
    pub fn add(&mut self, p: Pos2, line: bool) -> bool {
        let point = fraction(&self.boxed, p);
        let mut points = self.points.clone();
        if line {
            points.truncate(1);
        }
        if points.last() != Some(&point) {
            points.push(point);
        }
        if points == self.points {
            return false;
        }
        self.points = points;
        self.drafted = true;
        true
    }

    /// `drawing add <id> --points … --width … (--color … | --tool eraser) [--at <key>]`.
    pub fn step(&self) -> Step {
        let points: Vec<String> = self.points.iter().flatten().map(f64::to_string).collect();
        let paint = match self.tool {
            StrokeTool::Pencil => ("color", json!(self.color)),
            StrokeTool::Eraser => ("tool", json!(self.tool.name())),
        };
        self.edit.step(
            "drawing add",
            [
                ("points", json!(points.join(","))),
                ("width", json!(self.width)),
                paint,
            ],
        )
    }
}

/// `p` (canvas px) in fractions of `boxed` along its axes, kept inside it, to 4 decimals.
fn fraction(boxed: &Boxed, p: Pos2) -> [f64; 2] {
    let v = boxed.local(p) / boxed.size;
    let part = |v: f32| (f64::from(v + 0.5).clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0;
    [part(v.x), part(v.y)]
}

/// Annotation `id` of `project` is a drawing.
pub fn is_drawing(project: &Project, id: AnnotId) -> bool {
    project.state::<Annotations>().is_ok_and(|state| {
        state
            .annotations
            .iter()
            .any(|a| a.id == id && a.kind == Kind::Drawing)
    })
}

/// The brush bar over the top of the preview `area`: «Карандаш | Ластик» chooses the tool of a
/// left drag, the slider the eraser's thickness. It lies above the frame, so a press on it never
/// starts a stroke.
pub fn bar(ui: &Ui, brush: &mut Brush, area: Rect) {
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
                    segmented(
                        ui,
                        &mut brush.tool,
                        &[
                            (StrokeTool::Pencil, text::BRUSH_PENCIL, true),
                            (StrokeTool::Eraser, text::BRUSH_ERASER, true),
                        ],
                    );
                    ui.label(RichText::new(text::ERASER_SIZE).size(FS_S).color(TEXT_2));
                    Slider::new(&mut brush.eraser, WIDTH_MIN..=WIDTH_MAX, text::ERASER_SIZE)
                        .width(96.0)
                        .suffix(text::PX)
                        .show(ui);
                });
            });
        });
}
