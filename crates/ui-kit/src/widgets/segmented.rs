//! Segmented choice of the mockup (`.seg`): frame rate 30/60/120, countdown «Нет/3 с/5 с/10 с»,
//! area presets. One option is always chosen.

use egui::{Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::fonts::{self, Weight};
use crate::tokens::{BORDER, FS_S, R1, SURFACE_2, SURFACE_3, TEXT, TEXT_2};

const PAD: egui::Vec2 = vec2(10.0, 3.0);
const INSET: f32 = 2.0;

/// Shows the `options` (value and caption) in a row; returns the response of the clicked
/// option (changed) or of the whole group. Disabled options (`enabled` false) are faded.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str, bool)],
) -> Response {
    let font = fonts::text(FS_S, Weight::Medium);
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, text, _)| {
            ui.painter()
                .layout_no_wrap((*text).to_owned(), font.clone(), TEXT)
        })
        .collect();
    let height = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max) + 2.0 * (PAD.y + INSET);
    let width: f32 = galleys
        .iter()
        .map(|g| g.size().x + 2.0 * PAD.x + INSET)
        .sum::<f32>()
        + INSET;
    let (rect, mut group) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter().rect(
        rect,
        R1,
        SURFACE_2,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    let mut x = rect.left() + INSET;
    for ((option, text, enabled), galley) in options.iter().zip(galleys) {
        let item = Rect::from_min_size(
            egui::pos2(x, rect.top() + INSET),
            vec2(galley.size().x + 2.0 * PAD.x, height - 2.0 * INSET),
        );
        x = item.right() + INSET;
        let sense = if *enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let mut response = ui.interact(item, group.id.with(text), sense);
        let selected = *value == *option;
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::RadioButton, *enabled, selected, *text)
        });
        if response.clicked() && !selected {
            *value = *option;
            response.mark_changed();
            group = response.clone();
        }
        let color = match (selected, *enabled) {
            (true, _) => TEXT,
            (false, true) if response.hovered() => TEXT,
            (false, true) => TEXT_2,
            (false, false) => TEXT_2.gamma_multiply(0.45),
        };
        if *value == *option {
            ui.painter().rect_filled(item, 4, SURFACE_3);
        }
        let top = item.center().y - galley.size().y / 2.0;
        ui.painter()
            .galley(egui::pos2(item.left() + PAD.x, top), galley, color);
    }
    group
}
