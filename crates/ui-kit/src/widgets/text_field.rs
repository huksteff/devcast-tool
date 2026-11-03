//! One-line text field: 32 px high, 10 px inside, grey hint; the hint is also its accessible
//! name, so kittest finds the field by it. The search field has the search icon at
//! its left (mockup 5b).

use egui::{Align2, Margin, Response, TextEdit, Ui, WidgetInfo, vec2};

use crate::icons::Icon;
use crate::tokens::{CONTROL, ICON, S2, TEXT_3};

/// Inner left and right padding, points (mockup: 10 px).
const PAD: f32 = 10.0;
/// Gap between the icon and the text, points (mockup: 6 px).
const ICON_GAP: f32 = 6.0;

pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    field(ui, text, hint, width, None)
}

/// A text field with the search icon before the text.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    field(ui, text, hint, width, Some(Icon::Search))
}

/// A text of several lines (mockup `.txtarea`), `rows` lines high at least: Enter starts a new
/// line, the field grows with the text.
pub fn text_area(ui: &mut Ui, text: &mut String, hint: &str, width: f32, rows: usize) -> Response {
    let response = ui.add(
        TextEdit::multiline(text)
            .hint_text(hint.to_owned())
            .desired_width(width)
            .desired_rows(rows)
            .margin(Margin::symmetric(PAD as i8, S2 as i8)),
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::text_edit(ui.is_enabled(), text.as_str(), text.as_str(), hint);
        info.label = Some(hint.to_owned());
        info
    });
    response
}

fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32, icon: Option<Icon>) -> Response {
    let left = PAD + icon.map_or(0.0, |_| ICON + ICON_GAP);
    let response = ui.add_sized(
        [width, CONTROL],
        TextEdit::singleline(text)
            .hint_text(hint.to_owned())
            .margin(Margin {
                left: left as i8,
                right: PAD as i8,
                top: 0,
                bottom: 0,
            })
            .align(Align2::LEFT_CENTER),
    );
    if let Some(icon) = icon {
        let at = egui::pos2(
            response.rect.left() + PAD,
            response.rect.center().y - ICON / 2.0,
        );
        icon.paint(
            ui.painter(),
            egui::Rect::from_min_size(at, vec2(ICON, ICON)),
            TEXT_3,
        );
    }
    response.widget_info(|| {
        let mut info = WidgetInfo::text_edit(ui.is_enabled(), text.as_str(), text.as_str(), hint);
        info.label = Some(hint.to_owned());
        info
    });
    response
}
