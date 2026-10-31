//! Rows of the settings window (mockup `.field`): a caption with an optional grey hint under it
//! on the left, the control on the right, both centred vertically; a thin line under each row.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::tokens::{CONTROL, FS_L, FS_M, S1, S3, SURFACE_2, TEXT, TEXT_2, TEXT_3};
use eframe::egui::{self, Align, Layout, RichText, Stroke, Ui, vec2};

/// Width of the caption column, points (mockup: 190 px).
pub const LABEL: f32 = 190.0;
/// Size of the hint under a caption, points (mockup: 11 px).
const HINT: f32 = 11.0;

pub fn heading(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .font(fonts::text(FS_L, Weight::Semibold))
            .color(TEXT),
    );
    ui.add_space(S3);
}

/// A row of `height` whose widgets are centred vertically, left to right.
pub fn row<R>(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    let layout = Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(vec2(width, height), layout, |ui| {
        ui.set_min_size(vec2(width, height));
        add(ui)
    })
    .inner
}

/// Height of a row holding a control and a caption of `caption` height: a 4 px margin above and
/// below, so the line in the middle of the 8 px gap stands 8 px from the control (mockup).
pub fn row_height(caption: f32) -> f32 {
    caption.max(CONTROL) + 2.0 * S1
}

/// The thin line under the last row, in the middle of the gap below it.
pub fn separator(ui: &Ui) {
    let y = ui.cursor().top() - ui.spacing().item_spacing.y / 2.0;
    let x = ui.max_rect().x_range();
    ui.painter().hline(x, y, Stroke::new(1.0, SURFACE_2));
}

/// A caption of `LABEL` width and the control after it.
pub fn field(ui: &mut Ui, label: &str, hint: Option<&str>, value: impl FnOnce(&mut Ui)) {
    let height = row_height(caption_height(ui, label, hint, LABEL));
    row(ui, height, |ui| {
        caption(ui, label, hint, LABEL);
        ui.add_space(S3 - ui.spacing().item_spacing.x);
        value(ui);
    });
    separator(ui);
}

/// The caption takes the row, the control of `control_width` stands at the right edge
/// (mockup 5c: `grid-template-columns: 1fr auto`).
pub fn wide_field(
    ui: &mut Ui,
    label: &str,
    hint: Option<&str>,
    control_width: f32,
    value: impl FnOnce(&mut Ui),
) {
    let width = ui.available_width() - S3 - control_width;
    let height = row_height(caption_height(ui, label, hint, width));
    row(ui, height, |ui| {
        caption(ui, label, hint, width);
        ui.with_layout(Layout::right_to_left(Align::Center), value);
    });
    separator(ui);
}

fn label_text(label: &str) -> RichText {
    RichText::new(label)
        .font(fonts::text(FS_M, Weight::Regular))
        .color(TEXT_2)
}

fn hint_text(hint: &str) -> RichText {
    RichText::new(hint)
        .font(fonts::text(HINT, Weight::Regular))
        .color(TEXT_3)
}

/// Height of the caption and its hint wrapped to `width`.
fn caption_height(ui: &Ui, label: &str, hint: Option<&str>, width: f32) -> f32 {
    let painter = ui.painter();
    let mut height = painter
        .layout(
            label.to_owned(),
            fonts::text(FS_M, Weight::Regular),
            TEXT_2,
            width,
        )
        .size()
        .y;
    if let Some(hint) = hint {
        let font = fonts::text(HINT, Weight::Regular);
        height += painter
            .layout(hint.to_owned(), font, TEXT_3, width)
            .size()
            .y;
    }
    height
}

fn caption(ui: &mut Ui, label: &str, hint: Option<&str>, width: f32) {
    let height = caption_height(ui, label, hint, width);
    let layout = Layout::top_down(Align::Min);
    ui.allocate_ui_with_layout(vec2(width, height), layout, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add(egui::Label::new(label_text(label)).wrap());
        if let Some(hint) = hint {
            ui.add(egui::Label::new(hint_text(hint)).wrap());
        }
    });
}
