//! «Основное» of a text annotation (main window «Текст»): the words, font and size, B I U and
//! alignment, the colours of the letters and of the plate (`text set`); «Тонкая настройка» —
//! the corner radius of the plate. «Сбросить» brings the look to the defaults and keeps the
//! words.

use dc_fx_text::{RADIUS_MAX, SIZE_MAX, SIZE_MIN, TextAlign, TextBody};
use dc_ops::Step;
use dc_render::FAMILIES;
use dc_ui_kit::strings::annotations as text;
use dc_ui_kit::widgets::{IconButton, dropdown};
use eframe::egui::Ui;
use serde_json::{Value, json};

use super::annotation::{Edit, Section};
use super::{BUTTON, CHEVRON, Cx, rows};

const SET: &str = "text set";

const ALIGNS: [(TextAlign, &str); 3] = [
    (TextAlign::Left, text::LEFT),
    (TextAlign::Center, text::CENTER),
    (TextAlign::Right, text::RIGHT),
];

impl Section for TextBody {
    const ID: &'static str = "text";

    fn basic(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        let apply = |typed: String| edit.step(SET, [("text", json!(typed))]);
        if let Some(step) = rows::text_area(ui, text::TEXT_HINT, edit.id, &self.text, apply) {
            cx.actions.push(crate::editor::Action::Apply(vec![step]));
        }
        let families: Vec<(String, &str)> =
            FAMILIES.iter().map(|f| ((*f).to_owned(), *f)).collect();
        let font = rows::row(ui, text::FONT, |ui| {
            let mut chosen = self.font.clone();
            let width = ui.available_width();
            dropdown(ui, text::FONT, &mut chosen, &families, width)
                .changed()
                .then_some(chosen)
        });
        if let Some(font) = font {
            cx.actions.push(edit.apply(SET, [("font", json!(font))]));
        }
        let size = (1.0, text::PX);
        if let Some(v) = rows::slider(ui, text::SIZE, self.size, SIZE_MIN..=SIZE_MAX, size) {
            cx.actions.push(edit.apply(SET, [("size", json!(v))]));
        }
        if let Some((field, on)) = style(ui, self) {
            cx.actions.push(edit.apply(SET, [(field, json!(on))]));
        }
        if let Some(align) = rows::choice_under(ui, text::ALIGN, Some(self.align), &ALIGNS) {
            cx.actions.push(edit.apply(SET, [("align", json!(align))]));
        }
        let colors = [(text::COLOR, self.color), (text::PLATE, self.background)];
        if let Some((i, color)) = rows::colors(ui, text::COLORS, &colors) {
            let field = if i == 0 { "color" } else { "background" };
            cx.actions.push(edit.apply(SET, [(field, json!(color))]));
        }
        rows::note(ui, text::PLATE_NOTE);
        rows::note(ui, text::TEXT_NOTE);
    }

    fn basic_reset(&self, edit: Edit) -> Option<Vec<Step>> {
        let look = edit.reset(SET, &look(self), &look(&TextBody::default()));
        Some(look.into_iter().collect())
    }

    fn fine(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        let radius = (1.0, text::PX);
        if let Some(v) = rows::slider(ui, text::RADIUS, self.radius, 0.0..=RADIUS_MAX, radius) {
            cx.actions.push(edit.apply(SET, [("radius", json!(v))]));
        }
    }

    fn fine_reset(&self, edit: Edit) -> Vec<Step> {
        let default = TextBody::default().radius;
        let now = [("radius", json!(self.radius))];
        edit.reset(SET, &now, &[("radius", json!(default))])
            .into_iter()
            .collect()
    }
}

/// B, I and U side by side: the field switched and its new value.
fn style(ui: &mut Ui, body: &TextBody) -> Option<(&'static str, bool)> {
    rows::row(ui, text::STYLE, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let faces = [
            ("bold", "B", text::BOLD, body.bold),
            ("italic", "I", text::ITALIC, body.italic),
            ("underline", "U", text::UNDERLINE, body.underline),
        ];
        let mut switched = None;
        for (field, face, label, on) in faces {
            let button = IconButton::text(face, label).size(BUTTON, CHEVRON).on(on);
            if ui.add(button).on_hover_text(label).clicked() {
                switched = Some((field, !on));
            }
        }
        switched
    })
}

/// The look of «Основное» as arguments of `text set`; the words are not a look.
fn look(b: &TextBody) -> [(&'static str, Value); 8] {
    [
        ("font", json!(b.font)),
        ("size", json!(b.size)),
        ("bold", json!(b.bold)),
        ("italic", json!(b.italic)),
        ("underline", json!(b.underline)),
        ("align", json!(b.align)),
        ("color", json!(b.color)),
        ("background", json!(b.background)),
    ]
}
