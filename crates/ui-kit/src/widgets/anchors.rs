//! Place in the frame chosen on a small grid of the mockup (`.anchors`): cells 22 × 16 three in a
//! row, the chosen one blue. Each cell is a radio button named by its place («Внизу справа»).

use egui::{Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::tokens::{ACCENT_HI, SURFACE_3, TEXT_3};

const CELL: egui::Vec2 = vec2(22.0, 16.0);
const GAP: f32 = 3.0;
const RADIUS: f32 = 3.0;

/// Shows a cell per name of `places`, `columns` in a row; `value` is the index of the chosen one.
/// Returns the response of the clicked cell (changed) or of the whole grid.
pub fn anchors(ui: &mut Ui, value: &mut usize, columns: usize, places: &[&str]) -> Response {
    let columns = columns.max(1);
    let rows = places.len().div_ceil(columns);
    let size = vec2(
        columns as f32 * (CELL.x + GAP) - GAP,
        rows as f32 * (CELL.y + GAP) - GAP,
    );
    let (rect, mut grid) = ui.allocate_exact_size(size, Sense::hover());
    for (i, place) in places.iter().enumerate() {
        let (row, column) = (i / columns, i % columns);
        let min = pos2(
            rect.left() + column as f32 * (CELL.x + GAP),
            rect.top() + row as f32 * (CELL.y + GAP),
        );
        let cell = Rect::from_min_size(min, CELL);
        let mut response = ui.interact(cell, grid.id.with(i), Sense::click());
        let chosen = *value == i;
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::RadioButton, ui.is_enabled(), chosen, *place)
        });
        if response.clicked() && !chosen {
            *value = i;
            response.mark_changed();
            grid = response.clone();
        }
        let fill = match (*value == i, response.hovered()) {
            (true, _) => ACCENT_HI,
            (false, true) => TEXT_3,
            (false, false) => SURFACE_3,
        };
        ui.painter().rect_filled(cell, RADIUS, fill);
    }
    grid
}
