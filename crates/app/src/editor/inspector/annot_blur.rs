//! «Основное» of a blur (mockup 3c «Размытие»): strength, the tint — its colour and how much it
//! covers (the alpha of `tint`, 100 % a solid fill) — and the corner radius (`blur set`).
//! The box turns with «Поворот» of «Тонкая настройка», like every annotation.

use dc_fx_blur::{BlurBody, RADIUS_MAX, STRENGTH_MAX, STRENGTH_MIN};
use dc_ops::Step;
use dc_render::Rgba;
use dc_ui_kit::strings::{annotations as text, inspector as names};
use eframe::egui::Ui;
use serde_json::{Value, json};

use super::annotation::{Edit, Section};
use super::{Cx, rows};

const SET: &str = "blur set";

impl Section for BlurBody {
    const ID: &'static str = "blur";

    fn basic(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        let range = STRENGTH_MIN..=STRENGTH_MAX;
        if let Some(v) = rows::slider(ui, text::STRENGTH, self.strength, range, (1.0, "")) {
            cx.actions.push(edit.apply(SET, [("strength", json!(v))]));
        }
        let Rgba([r, g, b, a]) = self.tint;
        let opaque = Rgba([r, g, b, 0xff]);
        if let Some((_, Rgba([r, g, b, _]))) =
            rows::colors(ui, text::TINT, &[(text::TINT_COLOR, opaque)])
        {
            cx.actions
                .push(edit.apply(SET, [("tint", json!(Rgba([r, g, b, a])))]));
        }
        let cover = f64::from(a) / 255.0 * 100.0;
        let percent = (1.0, names::PERCENT);
        if let Some(v) = rows::slider(ui, text::TINT_COVER, cover.round(), 0.0..=100.0, percent) {
            let alpha = (v / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
            cx.actions
                .push(edit.apply(SET, [("tint", json!(Rgba([r, g, b, alpha])))]));
        }
        rows::note(ui, text::TINT_NOTE);
        let px = (1.0, text::PX);
        if let Some(v) = rows::slider(ui, text::RADIUS, self.radius, 0.0..=RADIUS_MAX, px) {
            cx.actions.push(edit.apply(SET, [("radius", json!(v))]));
        }
    }

    fn basic_reset(&self, edit: Edit) -> Option<Vec<Step>> {
        let steps = edit.reset(SET, &fields(self), &fields(&BlurBody::default()));
        Some(steps.into_iter().collect())
    }
}

fn fields(b: &BlurBody) -> [(&'static str, Value); 3] {
    [
        ("strength", json!(b.strength)),
        ("tint", json!(b.tint)),
        ("radius", json!(b.radius)),
    ]
}
