//! «Основное» of a drawing (mockup 3c «Рисунок»): the colour and thickness of every pencil
//! stroke and the opacity of the drawing (`drawing set`), how many strokes it has and «Очистить
//! рисунок» (`drawing clear`); strokes are drawn and erased on the preview (`stage/brush.rs`).
//! «Сбросить» brings the pencil to blue 4 px and the opacity to 100 %.

use dc_fx_drawing::{DrawingBody, PENCIL_WIDTH, WIDTH_MAX, WIDTH_MIN};
use dc_ops::Step;
use dc_render::{BLUE, Rgba};
use dc_ui_kit::strings::{annotations as text, inspector as names};
use dc_ui_kit::widgets::{Button, Kind};
use eframe::egui::Ui;
use serde_json::{Value, json};

use super::annotation::{Edit, Section};
use super::{Cx, rows};

const SET: &str = "drawing set";

impl Section for DrawingBody {
    const ID: &'static str = "drawing";

    fn basic(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        let (color, width) = self.pencil();
        if let Some((_, color)) = rows::colors(ui, text::DRAWING_COLOR, &[(text::PENCIL, color)]) {
            cx.actions.push(edit.apply(SET, [("color", json!(color))]));
        }
        let px = (1.0, text::PX);
        if let Some(v) = rows::slider(ui, text::THICKNESS, width, WIDTH_MIN..=WIDTH_MAX, px) {
            cx.actions.push(edit.apply(SET, [("width", json!(v))]));
        }
        let percent = (1.0, names::PERCENT);
        let opacity = self.opacity * 100.0;
        if let Some(v) = rows::slider(ui, text::OPACITY, opacity, 0.0..=100.0, percent) {
            cx.actions
                .push(edit.apply(SET, [("opacity", json!(v / 100.0))]));
        }
        rows::note(ui, &text::strokes(self.strokes.len()));
        let clear = Button::new(text::CLEAR).kind(Kind::Ghost).small();
        if ui.add_enabled(!self.strokes.is_empty(), clear).clicked() {
            cx.actions.push(edit.apply("drawing clear", []));
        }
        rows::note(ui, text::DRAWING_NOTE);
    }

    fn basic_reset(&self, edit: Edit) -> Option<Vec<Step>> {
        let default = fields((BLUE, PENCIL_WIDTH), 1.0);
        let now = fields(self.pencil(), self.opacity);
        Some(edit.reset(SET, &now, &default).into_iter().collect())
    }
}

fn fields((color, width): (Rgba, f64), opacity: f64) -> [(&'static str, Value); 3] {
    [
        ("color", json!(color)),
        ("width", json!(width)),
        ("opacity", json!(opacity)),
    ]
}
