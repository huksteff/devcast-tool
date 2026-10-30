//! One-line text field: 32 px high, 10 px inside, grey hint; the hint is also its accessible
//! name, so kittest and egui-mcp find the field by it.

use egui::{Align2, Margin, Response, TextEdit, Ui, WidgetInfo};

use crate::tokens::CONTROL;

pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let response = ui.add_sized(
        [width, CONTROL],
        TextEdit::singleline(text)
            .hint_text(hint.to_owned())
            .margin(Margin::symmetric(10, 0))
            .align(Align2::LEFT_CENTER),
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::text_edit(ui.is_enabled(), text.as_str(), text.as_str(), hint);
        info.label = Some(hint.to_owned());
        info
    });
    response
}
