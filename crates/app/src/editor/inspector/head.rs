//! The head of a panel (mockup `.ph`): the colour chip of its effect, the title, the subtitle
//! and buttons on the right.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::tokens::{self, FS_L, FS_S, Hue, S2, S3, TEXT, TEXT_3};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{self, Align, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use super::{BUTTON, CHEVRON, PANEL_HEADER, bottom_line};

/// The head of a panel: the colour of its effect, its name, what it applies to.
pub struct Header<'a> {
    pub hue: Hue,
    pub title: &'a str,
    pub subtitle: &'a str,
}

/// The colour chip, the title and the subtitle; `buttons` lays out its buttons from the right.
pub fn header(ui: &mut Ui, head: &Header<'_>, buttons: impl FnOnce(&mut Ui)) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), PANEL_HEADER), Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Heading, true, head.title);
        info.current_text_value = Some(head.subtitle.to_owned());
        info
    });
    let painter = ui.painter();
    let chip = Rect::from_center_size(
        pos2(rect.left() + S3 + 2.0, rect.center().y),
        vec2(4.0, 22.0),
    );
    painter.rect_filled(chip, 2, head.hue.line);
    let title = painter.layout_no_wrap(
        head.title.to_owned(),
        fonts::text(FS_L, Weight::Semibold),
        TEXT,
    );
    let mut height = title.size().y;
    let subtitle = (!head.subtitle.is_empty()).then(|| {
        let galley = painter.layout_no_wrap(
            head.subtitle.to_owned(),
            fonts::text(FS_S, Weight::Regular),
            TEXT_3,
        );
        height += galley.size().y;
        galley
    });
    let mut at = pos2(chip.right() + S2, rect.center().y - height / 2.0);
    let line = title.size().y;
    painter.galley(at, title, TEXT);
    if let Some(subtitle) = subtitle {
        at.y += line;
        painter.galley(at, subtitle, TEXT_3);
    }
    let right = rect.shrink2(vec2(S2, 0.0));
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(right)
            .layout(Layout::right_to_left(Align::Center)),
    );
    cell.spacing_mut().item_spacing.x = 2.0;
    buttons(&mut cell);
    bottom_line(ui, rect);
}

/// A header button of [`header`] (delete, duplicate); `danger` draws it red (mockup `.ib.danger`).
pub fn header_button(ui: &mut Ui, icon: Icon, label: &str, danger: bool) -> bool {
    let mut button = IconButton::new(icon, label).size(BUTTON, CHEVRON);
    if danger {
        button = button.color(tokens::REC);
    }
    ui.add(button).on_hover_text(label).clicked()
}
