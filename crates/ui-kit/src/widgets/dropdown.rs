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
    let items: Vec<(T, &str, bool)> = items.iter().map(|(v, t)| (v.clone(), *t, true)).collect();
    dropdown_some(ui, label, value, &items, width)
}

/// [`dropdown`] whose items (value, caption, can be picked) may be grey: they stay in the list
/// with their reason in the caption, so the user sees what exists (mockup 3c «Печать · только
/// текст»).
pub fn dropdown_some<T: PartialEq + Clone>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    items: &[(T, &str, bool)],
    width: f32,
) -> Response {
    let chosen = items
        .iter()
        .find(|(item, _, _)| item == value)
        .map_or("", |(_, text, _)| *text);
    let mut changed = false;
    let mut response = egui::ComboBox::from_id_salt(label)
        .selected_text(chosen.to_owned())
        .width(width)
        .icon(|ui, rect, _, _| {
            let icon = Rect::from_center_size(rect.center(), vec2(ICON, ICON) * 0.75);
            Icon::Down.paint(ui.painter(), icon, TEXT_2);
        })
        .show_ui(ui, |ui| {
            for (item, text, can) in items {
                let picked = ui
                    .add_enabled_ui(*can, |ui| ui.selectable_value(value, item.clone(), *text))
                    .inner;
                changed |= picked.changed();
            }
        })
        .response;
    if changed {
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), label));
    response
}
