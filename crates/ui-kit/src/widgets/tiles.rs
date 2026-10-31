//! Tiles of the mockup (`.tiles`): equal tiles in one row over the whole width, a picture and a
//! caption under it; the chosen one framed in blue. One is always chosen. Each tile is a radio
//! button named by its caption.

use egui::{Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::fonts::{self, Weight};
use crate::tokens::{ACCENT_HI, ACCENT_SOFT, BORDER, R2, SURFACE_2, SURFACE_3, TEXT, TEXT_2};

const GAP: f32 = 6.0;
const HEIGHT: f32 = 54.0;
/// Side of the picture, gap above it and between it and the caption (mockup: 26, 6, 4).
const PICTURE: f32 = 26.0;
const TOP: f32 = 6.0;
const UNDER: f32 = 4.0;
/// Caption size, points (mockup: 11).
const CAPTION: f32 = 11.0;

/// Shows a tile per option (value and caption); `picture` draws the picture of a value into its
/// square. Returns the response of the clicked tile (changed) or of the whole row.
pub fn tiles<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str)],
    picture: impl Fn(&Painter, Rect, T),
) -> Response {
    let count = options.len().max(1) as f32;
    let width = ui.available_width();
    let tile = ((width - GAP * (count - 1.0)) / count).max(1.0);
    let (rect, mut row) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::hover());
    for (i, (option, caption)) in options.iter().enumerate() {
        let left = rect.left() + i as f32 * (tile + GAP);
        let cell = Rect::from_min_size(pos2(left, rect.top()), vec2(tile, HEIGHT));
        let mut response = ui.interact(cell, row.id.with(i), Sense::click());
        let chosen = *value == *option;
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::RadioButton, ui.is_enabled(), chosen, *caption)
        });
        if response.clicked() && !chosen {
            *value = *option;
            response.mark_changed();
            row = response.clone();
        }
        let (fill, edge, color) = match (*value == *option, response.hovered()) {
            (true, _) => (ACCENT_SOFT, ACCENT_HI, TEXT),
            (false, true) => (SURFACE_3, BORDER, TEXT),
            (false, false) => (SURFACE_2, BORDER, TEXT_2),
        };
        let painter = ui.painter();
        painter.rect(cell, R2, fill, Stroke::new(1.0, edge), StrokeKind::Inside);
        let square = Rect::from_center_size(
            pos2(cell.center().x, cell.top() + TOP + PICTURE / 2.0),
            vec2(PICTURE, PICTURE),
        );
        picture(painter, square, *option);
        let galley = painter.layout_no_wrap(
            (*caption).to_owned(),
            fonts::text(CAPTION, Weight::Medium),
            color,
        );
        let at = pos2(
            cell.center().x - galley.size().x / 2.0,
            square.bottom() + UNDER,
        );
        painter.with_clip_rect(cell).galley(at, galley, color);
    }
    row
}
