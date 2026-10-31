//! Parameter rows of a panel (mockup `.prow`): a 104 px caption and the control after it, laid out
//! by `settings_rows` like the settings window. Each helper returns the new value once the edit
//! is committed, and the panel turns it into one registry command — one undo step: a click or
//! a typed number (on Enter or when the field loses focus) at once, a slider or a colour dragged
//! with the pointer when it is let go. Meanwhile the control shows the dragged value (a draft in
//! egui's temporary memory) and the project stays as it was.

use dc_render::Rgba;
use dc_ui_kit::tokens::{FS_S, S2, TEXT_3};
use dc_ui_kit::widgets::{Slider, Toggle, anchors as anchor_grid, color_button};
use eframe::egui::{self, Color32, Id, Response, RichText, Ui};

use crate::settings_rows;

/// Width of the caption column, least height of a row, points (mockup `.prow`: 96, but Inter
/// here is wider and «Буквы · плашка» would wrap).
const LABEL: f32 = 104.0;
const ROW: f32 = 30.0;

/// A caption and its control; the control's answer.
fn row<T>(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui) -> T) -> T {
    settings_rows::labelled(ui, label, (LABEL, S2, ROW), control)
}

/// The edited value of the control `show` draws from `now`: a draft while the pointer is down
/// or a text field has the keys, the value once the edit ends (`None` when it did not change).
fn drafted<T>(
    ui: &mut Ui,
    id: Id,
    now: T,
    show: impl FnOnce(&mut Ui, &mut T) -> Response,
) -> Option<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    let draft = ui.data(|d| d.get_temp::<T>(id));
    let drafting = draft.is_some();
    let mut value = draft.unwrap_or_else(|| now.clone());
    let response = show(ui, &mut value);
    if !response.changed() && !drafting {
        return None;
    }
    let held = ui.input(|i| i.pointer.any_down()) || ui.ctx().text_edit_focused();
    if held {
        ui.data_mut(|d| d.insert_temp(id, value));
        return None;
    }
    ui.data_mut(|d| d.remove::<T>(id));
    (value != now).then_some(value)
}

/// An on/off switch.
pub fn toggle(ui: &mut Ui, label: &str, on: bool) -> Option<bool> {
    row(ui, label, |ui| {
        let mut value = on;
        ui.add(Toggle::new(&mut value, label))
            .changed()
            .then_some(value)
    })
}

/// A slider over `range` in steps of `step` with its number field and `suffix` (« с», «×»).
pub fn slider(
    ui: &mut Ui,
    label: &str,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    (step, suffix): (f64, &str),
) -> Option<f64> {
    row(ui, label, |ui| {
        let width = ui.available_width() - Slider::FIELD - S2;
        let id = ui.make_persistent_id(("draft", label));
        drafted(ui, id, value, |ui, v| {
            Slider::new(v, range, label)
                .width(width)
                .step(step)
                .suffix(suffix)
                .show(ui)
        })
    })
}

/// Swatches of `colors` (accessible name, colour) in a row; the index and the new colour of the
/// one changed.
pub fn colors(ui: &mut Ui, label: &str, colors: &[(&str, Rgba)]) -> Option<(usize, Rgba)> {
    row(ui, label, |ui| {
        let mut changed = None;
        for (i, (name, color)) in colors.iter().enumerate() {
            let id = ui.make_persistent_id(("draft", *name));
            let now = color32(*color);
            if let Some(new) = drafted(ui, id, now, |ui, c| color_button(ui, c, name)) {
                changed = Some((i, rgba(new)));
            }
        }
        changed
    })
}

/// A place on the anchor grid, three in a row: the index into `places`.
pub fn anchors(ui: &mut Ui, label: &str, value: usize, places: &[&str]) -> Option<usize> {
    row(ui, label, |ui| {
        let mut chosen = value;
        anchor_grid(ui, &mut chosen, 3, places)
            .changed()
            .then_some(chosen)
    })
}

/// A grey line of explanation under the rows (mockup `.hint`).
pub fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(FS_S).color(TEXT_3)).wrap());
}

fn color32(Rgba([r, g, b, a]): Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn rgba(color: Color32) -> Rgba {
    Rgba(color.to_srgba_unmultiplied())
}
