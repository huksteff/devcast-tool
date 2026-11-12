//! «Основное» of a shape (mockup 3c «Фигура»): the figure in tiles, the colours of the line and
//! the fill, the gradient, thickness, outline and opacity (`shape set`); «Тонкая настройка» —
//! glow, corner radius, the direction and loop of the gradient, the ends of an arrow or a line
//! in percent of the box (`--tail`, `--tip`). «Сбросить» brings the look of the figure to the defaults and keeps the
//! figure and its ends.

use crate::{CYCLE_MAX, Figure, GLOW_MAX, RADIUS_MAX, ShapeBody, WIDTH_MAX, WIDTH_MIN};
use dc_core::OpName;
use dc_core::Step;
use dc_render::Rgba;
use dc_ui_kit::egui::{Painter, Pos2, Rect, Stroke, Ui, pos2};
use dc_ui_kit::strings::{annotations as text, inspector as names};
use serde_json::{Value, json};

use dc_fx_annotations::panel::{Edit, Section};
use dc_ui_kit::panel::{Cx, rows};
use dc_ui_kit::tokens::TEXT_2;

const SET: OpName = OpName::of::<crate::ShapeSet>();

const FIGURES: [(Figure, &str); 6] = [
    (Figure::Arrow, text::ARROW),
    (Figure::Rectangle, text::RECTANGLE),
    (Figure::Ellipse, text::ELLIPSE),
    (Figure::Circle, text::CIRCLE),
    (Figure::Line, text::LINE),
    (Figure::Highlight, text::HIGHLIGHT),
];

impl Section for ShapeBody {
    const ID: &'static str = "shape";

    fn basic(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        // Two rows of three: six tiles in a row cut their captions.
        for half in FIGURES.chunks(3) {
            if let Some(figure) = rows::tiles(ui, self.figure, half, picture) {
                edit.apply(cx, SET, [("figure", json!(figure))]);
            }
        }
        let open = self.figure.is_open();
        let colors: Vec<(&str, Rgba)> = if open {
            vec![(text::STROKE, self.color)]
        } else {
            vec![(text::STROKE, self.color), (text::FILL, self.fill)]
        };
        if let Some((i, color)) = rows::colors(ui, text::SHAPE_COLORS, &colors) {
            let field = if i == 0 { "color" } else { "fill" };
            edit.apply(cx, SET, [(field, json!(color))]);
        }
        if let Some(on) = rows::toggle(ui, text::GRADIENT, self.gradient) {
            edit.apply(cx, SET, [("gradient", json!(on))]);
        }
        if self.gradient
            && let Some(colors) = rows::gradient(ui, text::GRADIENT, self.colors.colors())
        {
            edit.apply(cx, SET, [("colors", json!(joined(&colors)))]);
        }
        let px = (1.0, text::PX);
        if let Some(v) = rows::slider(ui, text::THICKNESS, self.width, WIDTH_MIN..=WIDTH_MAX, px) {
            edit.apply(cx, SET, [("width", json!(v))]);
        }
        if !open && let Some(on) = rows::toggle(ui, text::OUTLINE, self.outline) {
            edit.apply(cx, SET, [("outline", json!(on))]);
        }
        let percent = (1.0, names::PERCENT);
        let opacity = self.opacity * 100.0;
        if let Some(v) = rows::slider(ui, text::OPACITY, opacity, 0.0..=100.0, percent) {
            edit.apply(cx, SET, [("opacity", json!(v / 100.0))]);
        }
    }

    fn basic_reset(&self, edit: Edit) -> Option<Vec<Step>> {
        let default = ShapeBody::default().refigured(self.figure);
        let steps = edit.reset(SET, &look(self), &look(&default));
        Some(steps.into_iter().collect())
    }

    fn fine(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        let glow = (1.0, "");
        if let Some(v) = rows::slider(ui, text::GLOW, self.glow, 0.0..=GLOW_MAX, glow) {
            edit.apply(cx, SET, [("glow", json!(v))]);
        }
        if matches!(self.figure, Figure::Rectangle | Figure::Highlight) {
            let px = (1.0, text::PX);
            if let Some(v) = rows::slider(ui, text::RADIUS, self.radius, 0.0..=RADIUS_MAX, px) {
                edit.apply(cx, SET, [("radius", json!(v))]);
            }
        }
        if self.gradient {
            let turn = (1.0, text::DEGREES);
            if let Some(v) = rows::slider(ui, text::ANGLE, self.angle, -360.0..=360.0, turn) {
                edit.apply(cx, SET, [("angle", json!(v))]);
            }
            let loop_time = (0.05, names::SECONDS);
            if let Some(v) = rows::slider(ui, text::CYCLE, self.cycle, 0.0..=CYCLE_MAX, loop_time) {
                edit.apply(cx, SET, [("cycle", json!(v))]);
            }
            rows::note(ui, text::CYCLE_NOTE);
        }
        if self.figure.is_open() {
            ends(ui, cx, self, edit);
        }
    }

    fn fine_reset(&self, edit: Edit) -> Vec<Step> {
        let default = ShapeBody::default();
        let fields = |b: &ShapeBody| {
            [
                ("glow", json!(b.glow)),
                ("radius", json!(b.radius)),
                ("angle", json!(b.angle)),
                ("cycle", json!(b.cycle)),
            ]
        };
        edit.reset(SET, &fields(self), &fields(&default))
            .into_iter()
            .collect()
    }
}

/// The ends of an arrow or a line in percent of the box.
fn ends(ui: &mut Ui, cx: &mut Cx<'_>, body: &ShapeBody, edit: Edit) {
    let percent = (1.0, names::PERCENT);
    let points = [
        ("tail", body.from, [text::TAIL_X, text::TAIL_Y]),
        ("tip", body.to, [text::TIP_X, text::TIP_Y]),
    ];
    for (field, point, [x_label, y_label]) in points {
        let x = rows::slider(ui, x_label, point.x * 100.0, 0.0..=100.0, percent);
        let y = rows::slider(ui, y_label, point.y * 100.0, 0.0..=100.0, percent);
        let moved = match (x, y) {
            (Some(x), _) => Some((x / 100.0, point.y)),
            (_, Some(y)) => Some((point.x, y / 100.0)),
            _ => None,
        };
        if let Some((x, y)) = moved {
            edit.apply(cx, SET, [(field, json!(format!("{x},{y}")))]);
        }
    }
    rows::note(ui, text::ENDS_NOTE);
}

/// The look of «Основное» as arguments of `shape set`; the figure and its ends are not a look.
fn look(b: &ShapeBody) -> [(&'static str, Value); 7] {
    [
        ("color", json!(b.color)),
        ("width", json!(b.width)),
        ("outline", json!(b.outline)),
        ("fill", json!(b.fill)),
        ("gradient", json!(b.gradient)),
        ("colors", json!(joined(b.colors.colors()))),
        ("opacity", json!(b.opacity)),
    ]
}

/// `#ff4d9dff,#22d3eeff`: the colours as `shape set --colors` takes them.
fn joined(colors: &[Rgba]) -> String {
    let all: Vec<String> = colors.iter().map(Rgba::to_string).collect();
    all.join(",")
}

/// The picture of a figure tile, lines of the mockup's 24-unit icons.
fn picture(painter: &Painter, rect: Rect, figure: Figure) {
    let stroke = Stroke::new(1.5, TEXT_2);
    let at = |x: f32, y: f32| -> Pos2 {
        pos2(
            rect.left() + rect.width() * x / 24.0,
            rect.top() + rect.height() * y / 24.0,
        )
    };
    let square = |x, y, w, h| Rect::from_min_max(at(x, y), at(x + w, y + h));
    match figure {
        Figure::Arrow => {
            painter.line_segment([at(5.0, 19.0), at(19.0, 5.0)], stroke);
            painter.line(vec![at(10.0, 5.0), at(19.0, 5.0), at(19.0, 14.0)], stroke);
        }
        Figure::Rectangle => {
            painter.rect_stroke(
                square(4.0, 6.0, 16.0, 12.0),
                2,
                stroke,
                dc_ui_kit::egui::StrokeKind::Middle,
            );
        }
        Figure::Ellipse => {
            let centre = at(12.0, 12.0);
            let radius =
                dc_ui_kit::egui::vec2(rect.width() * 9.0 / 24.0, rect.height() * 6.0 / 24.0);
            painter.add(dc_ui_kit::egui::epaint::EllipseShape::stroke(
                centre, radius, stroke,
            ));
        }
        Figure::Circle => {
            painter.circle_stroke(at(12.0, 12.0), rect.width() * 8.0 / 24.0, stroke);
        }
        Figure::Line => {
            painter.line_segment([at(5.0, 19.0), at(19.0, 5.0)], stroke);
        }
        Figure::Highlight => {
            painter.rect_filled(square(4.0, 7.0, 16.0, 10.0), 2, TEXT_2.gamma_multiply(0.5));
        }
    }
}

/// The kind as the editor registers it: its panel section and fields.
pub const KIND: dc_fx_annotations::panel::Entry =
    dc_fx_annotations::panel::Entry::of::<ShapeBody>(dc_fx_annotations::Kind::Shape);
