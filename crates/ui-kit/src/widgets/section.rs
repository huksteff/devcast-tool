//! Collapsible group of settings: a header with a chevron; open or closed is kept in egui memory
//! by id (persisted, so with eframe `persistence` it survives a restart).

use egui::collapsing_header::CollapsingState;
use egui::{Rect, Sense, Ui, WidgetInfo, vec2};

use crate::fonts::{self, Weight};
use crate::icons::Icon;
use crate::tokens::{CONTROL, FS_M, ICON, R1, S2, SURFACE_2, TEXT, TEXT_2};

#[must_use = "show it with `.show(ui, ...)`"]
pub struct Section<'a> {
    title: &'a str,
    default_open: bool,
}

impl<'a> Section<'a> {
    /// The title is also the id of the remembered state: titles of one window differ.
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            default_open: true,
        }
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// The header and, when open, `body`; returns what `body` returned.
    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let id = ui.make_persistent_id(self.title);
        let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, self.default_open);
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::click());
        let open = state.is_open();
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), open, self.title)
        });
        if response.clicked() {
            state.toggle(ui);
        }
        if ui.is_rect_visible(rect) {
            if response.hovered() {
                ui.painter().rect_filled(rect, R1, SURFACE_2);
            }
            let icon = if open { Icon::Down } else { Icon::Right };
            let icon_rect = Rect::from_center_size(
                egui::pos2(rect.left() + S2 + ICON / 2.0, rect.center().y),
                vec2(ICON, ICON),
            );
            icon.paint(ui.painter(), icon_rect, TEXT_2);
            ui.painter().text(
                egui::pos2(icon_rect.right() + S2, rect.center().y),
                egui::Align2::LEFT_CENTER,
                self.title,
                fonts::text(FS_M, Weight::Semibold),
                TEXT,
            );
        }
        state
            .show_body_unindented(ui, body)
            .map(|inner| inner.inner)
    }
}
