//! Drop-down list of the mockup (`.select`): egui's combo box in our theme with the line
//! chevron; the chosen item is the button's text and accessible name.

use egui::{Rect, Response, Ui, WidgetInfo, vec2};

use crate::icons::Icon;
use crate::tokens::{ICON, TEXT_2};

/// Shows the `items` (value and caption) under the chosen one; `label` is the accessible name
/// and tells several drop-downs of one window apart. The response is changed when the user
/// picked another item.
pub fn dropdown<T: PartialEq + Clone>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    items: &[(T, &str)],
    width: f32,
) -> Response {
    let chosen = items
        .iter()
        .find(|(item, _)| item == value)
        .map_or("", |(_, text)| *text);
    let mut changed = false;
    let mut response = egui::ComboBox::from_id_salt(label)
        .selected_text(chosen.to_owned())
        .width(width)
        .icon(|ui, rect, _, _| {
            let icon = Rect::from_center_size(rect.center(), vec2(ICON, ICON) * 0.75);
            Icon::Down.paint(ui.painter(), icon, TEXT_2);
        })
        .show_ui(ui, |ui| {
            for (item, text) in items {
                changed |= ui.selectable_value(value, item.clone(), *text).changed();
            }
        })
        .response;
    if changed {
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), label));
    response
}
