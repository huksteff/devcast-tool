//! Colour choice: a swatch that opens egui's HSV picker with a hex field under it, and the
//! gradient editor of 2–4 colours spread evenly (SCOPE: background and fill gradients).

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::icons::Icon;
use crate::strings::kit;
use crate::tokens::{BORDER, R1, S2, TEXT};
use crate::widgets::{IconButton, text_field};

const SWATCH: f32 = 24.0;
/// Bounds of the gradient editor.
pub const GRADIENT_COLORS: std::ops::RangeInclusive<usize> = 2..=4;

/// Swatch of `color`; a click opens the picker. `label` is the accessible name.
pub fn color_button(ui: &mut Ui, color: &mut Color32, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let border = if response.hovered() { TEXT } else { BORDER };
        ui.painter().rect(
            rect,
            R1,
            *color,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
    }
    let picked = egui::Popup::from_toggle_button_response(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| picker(ui, color));
    if picked.is_some_and(|picked| picked.inner) {
        response.mark_changed();
    }
    response
}

/// HSV picker and hex field; true when the colour changed.
fn picker(ui: &mut Ui, color: &mut Color32) -> bool {
    let mut changed =
        egui::color_picker::color_picker_color32(ui, color, egui::color_picker::Alpha::OnlyBlend);
    let mut hex = color.to_hex();
    if text_field(ui, &mut hex, kit::HEX_HINT, 120.0).changed()
        && let Ok(parsed) = Color32::from_hex(&hex)
    {
        *color = parsed;
        changed = true;
    }
    changed
}

/// The gradient bar and a swatch per colour with «+» and «−». `colors` keeps
/// [`GRADIENT_COLORS`] items: «+» and «−» are off at the bounds.
pub fn gradient_editor(ui: &mut Ui, colors: &mut Vec<Color32>, width: f32) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, SWATCH), Sense::hover());
    paint_gradient(ui, rect, colors);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = S2;
        for (i, color) in colors.iter_mut().enumerate() {
            let label = format!("{} {}", kit::COLOR, i + 1);
            if color_button(ui, color, &label).changed() {
                response.mark_changed();
            }
        }
        let can_add = colors.len() < *GRADIENT_COLORS.end();
        let can_remove = colors.len() > *GRADIENT_COLORS.start();
        let add = ui.add_enabled(can_add, IconButton::new(Icon::Plus, kit::ADD_COLOR));
        if add.clicked() {
            let last = colors.last().copied().unwrap_or(Color32::WHITE);
            colors.push(last);
            response.mark_changed();
        }
        let remove = ui.add_enabled(can_remove, IconButton::new(Icon::Minus, kit::REMOVE_COLOR));
        if remove.clicked() {
            colors.pop();
            response.mark_changed();
        }
    });
    response
}

/// Left to right through `colors` at even steps.
fn paint_gradient(ui: &Ui, rect: Rect, colors: &[Color32]) {
    let mut mesh = Mesh::default();
    let last = colors.len().saturating_sub(1).max(1) as f32;
    for (i, color) in colors.iter().enumerate() {
        let x = egui::lerp(rect.left()..=rect.right(), i as f32 / last);
        for y in [rect.top(), rect.bottom()] {
            mesh.vertices.push(Vertex {
                pos: pos2(x, y),
                uv: WHITE_UV,
                color: *color,
            });
        }
        if i > 0 {
            let base = 2 * i as u32;
            mesh.add_triangle(base - 2, base - 1, base);
            mesh.add_triangle(base - 1, base, base + 1);
        }
    }
    ui.painter().add(mesh);
    ui.painter()
        .rect_stroke(rect, 0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
}
