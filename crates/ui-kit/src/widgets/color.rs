//! Colour choice: a swatch that opens egui's HSV picker with a hex field under it, a palette of
//! swatches with an own colour after them, and the gradient editor of 2–4 colours spread evenly
//! (SCOPE: background and fill gradients).

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::icons::Icon;
use crate::strings::kit;
use crate::tokens::{BORDER, R1, S2, TEXT};
use crate::widgets::{IconButton, text_field};

const SWATCH: f32 = 24.0;
/// Side, corner radius and gap of a palette swatch, points (mockup `.cs`: 22, 5, 6 — 2 and 1 less
/// here, so that six swatches fit the 160 points an inspector row leaves after its caption).
const CHIP: f32 = 20.0;
const CHIP_RADIUS: u8 = 5;
const CHIP_GAP: f32 = 5.0;
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

/// A swatch per colour of `swatches` (colour, accessible name), then the own colour (`own` is its
/// name): a rainbow while `color` is one of the swatches, else that colour; a click opens the
/// picker. The chosen swatch is framed in white. Returns the response of the swatch that changed
/// `color` or of the whole row.
pub fn palette(
    ui: &mut Ui,
    color: &mut Color32,
    swatches: &[(Color32, &str)],
    own: &str,
) -> Response {
    let count = swatches.len() as f32 + 1.0;
    let size = vec2(count * (CHIP + CHIP_GAP) - CHIP_GAP, CHIP);
    let (rect, mut row) = ui.allocate_exact_size(size, Sense::hover());
    let chip = |i: usize| {
        let left = rect.left() + i as f32 * (CHIP + CHIP_GAP);
        Rect::from_min_size(pos2(left, rect.top()), vec2(CHIP, CHIP))
    };
    let mut in_palette = false;
    for (i, (swatch, name)) in swatches.iter().enumerate() {
        let mut response = ui.interact(chip(i), row.id.with(i), Sense::click());
        let chosen = *color == *swatch;
        in_palette |= chosen;
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::RadioButton, ui.is_enabled(), chosen, *name)
        });
        if response.clicked() && !chosen {
            *color = *swatch;
            response.mark_changed();
            row = response.clone();
        }
        ui.painter().rect_filled(chip(i), CHIP_RADIUS, *swatch);
        chip_edge(ui, chip(i), chosen, response.hovered());
    }
    let last = chip(swatches.len());
    let mut response = ui.interact(last, row.id.with("own"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), own));
    if in_palette {
        rainbow(ui, last);
    } else {
        ui.painter().rect_filled(last, CHIP_RADIUS, *color);
    }
    chip_edge(ui, last, !in_palette, response.hovered());
    let picked = egui::Popup::from_toggle_button_response(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| picker(ui, color));
    if picked.is_some_and(|picked| picked.inner) {
        response.mark_changed();
        row = response;
    }
    row
}

/// A white frame outside the chosen swatch, a faint edge inside the others.
fn chip_edge(ui: &Ui, rect: Rect, chosen: bool, hovered: bool) {
    let painter = ui.painter();
    if chosen {
        let stroke = Stroke::new(2.0, Color32::WHITE);
        painter.rect_stroke(rect, CHIP_RADIUS, stroke, StrokeKind::Outside);
    } else {
        let edge = if hovered { TEXT } else { BORDER };
        painter.rect_stroke(
            rect,
            CHIP_RADIUS,
            Stroke::new(1.0, edge),
            StrokeKind::Inside,
        );
    }
}

/// The swatch of an own colour: the hues around its centre.
fn rainbow(ui: &Ui, rect: Rect) {
    const SECTORS: u32 = 24;
    let mut mesh = Mesh::default();
    let centre = rect.center();
    mesh.vertices.push(Vertex {
        pos: centre,
        uv: WHITE_UV,
        color: Color32::WHITE,
    });
    let reach = rect.width() * 0.75;
    for i in 0..=SECTORS {
        let turn = i as f32 / SECTORS as f32;
        let angle = turn * std::f32::consts::TAU;
        mesh.vertices.push(Vertex {
            pos: centre + reach * vec2(angle.cos(), angle.sin()),
            uv: WHITE_UV,
            color: egui::ecolor::Hsva::new(turn, 1.0, 1.0, 1.0).into(),
        });
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    ui.painter().with_clip_rect(rect.shrink(1.0)).add(mesh);
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
