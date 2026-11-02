//! Text button of the mockup (`.btn`): 32 px high (small: 26), radius 6, medium 13 px text,
//! an optional icon before the text.

use egui::{Color32, Response, Sense, Stroke, StrokeKind, Ui, Widget, WidgetInfo, vec2};

use crate::fonts::{self, Weight};
use crate::icons::Icon;
use crate::tokens::{
    ACCENT, ACCENT_HI, BORDER, CONTROL, CONTROL_S, FS_M, FS_S, ICON, R1, REC, REC_STRONG, S2, S3,
    SURFACE_2, SURFACE_3, TEXT, TEXT_2, WHITE,
};

/// Side of the menu chevron, points (12 px in the mockup).
const CHEVRON: f32 = 12.0;

/// What the button means: its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Grey with a border: most buttons.
    Secondary,
    /// Blue: the main action of a dialog.
    Primary,
    /// Red: deleting and other dangerous actions.
    Danger,
    /// No background until hovered: minor actions in headers.
    Ghost,
}

#[must_use = "add it with `ui.add(...)`"]
pub struct Button<'a> {
    text: &'a str,
    kind: Kind,
    small: bool,
    icon: Option<Icon>,
    /// Room for an icon without one: menu items line up with those that have an icon.
    icon_room: bool,
    menu: bool,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            kind: Kind::Secondary,
            small: false,
            icon: None,
            icon_room: false,
            menu: false,
        }
    }

    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = kind;
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The room of an icon before the text, without an icon (a menu item among items with icons).
    pub fn icon_room(mut self) -> Self {
        self.icon_room = true;
        self
    }

    /// A chevron after the text: the button opens a menu.
    pub fn menu(mut self) -> Self {
        self.menu = true;
        self
    }

    /// The size the button takes, points: to lay out what stands next to it first.
    pub fn size(&self, ui: &Ui) -> egui::Vec2 {
        self.layout(ui).0
    }

    /// Size, side padding, icon width and the laid out text.
    fn layout(&self, ui: &Ui) -> (egui::Vec2, f32, f32, std::sync::Arc<egui::Galley>) {
        let (height, padding, size) = if self.small {
            (CONTROL_S, S2, FS_S)
        } else {
            (CONTROL, S3, FS_M)
        };
        let galley = ui.painter().layout_no_wrap(
            self.text.to_owned(),
            fonts::text(size, Weight::Medium),
            TEXT,
        );
        let icon_width = if self.icon.is_some() || self.icon_room {
            ICON + 6.0
        } else {
            0.0
        };
        let chevron = if self.menu { CHEVRON + 4.0 } else { 0.0 };
        let width = galley.size().x + icon_width + chevron + 2.0 * padding;
        (vec2(width, height), padding, icon_width, galley)
    }
}

/// Fill, border and text colour of `kind` when hovered or not.
fn colors(kind: Kind, hovered: bool) -> (Color32, Color32, Color32) {
    match (kind, hovered) {
        (Kind::Secondary, false) => (SURFACE_2, BORDER, TEXT),
        (Kind::Secondary, true) => (SURFACE_3, BORDER, TEXT),
        (Kind::Primary, false) => (ACCENT, ACCENT, WHITE),
        (Kind::Primary, true) => (ACCENT_HI, ACCENT_HI, WHITE),
        (Kind::Danger, false) => (REC_STRONG, REC_STRONG, WHITE),
        (Kind::Danger, true) => (REC, REC, WHITE),
        (Kind::Ghost, false) => (Color32::TRANSPARENT, Color32::TRANSPARENT, TEXT_2),
        (Kind::Ghost, true) => (SURFACE_3, SURFACE_3, TEXT),
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (size, padding, icon_width, galley) = self.layout(ui);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        response
            .widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), self.text));
        if ui.is_rect_visible(rect) {
            let (fill, border, text) = colors(self.kind, response.hovered());
            let painter = ui.painter();
            painter.rect(rect, R1, fill, Stroke::new(1.0, border), StrokeKind::Inside);
            let mut x = rect.left() + padding;
            if let Some(icon) = self.icon {
                let icon_rect = egui::Rect::from_min_size(
                    egui::pos2(x, rect.center().y - ICON / 2.0),
                    vec2(ICON, ICON),
                );
                icon.paint(painter, icon_rect, text);
            }
            x += icon_width;
            let top = rect.center().y - galley.size().y / 2.0;
            let text_right = x + galley.size().x;
            painter.galley(egui::pos2(x, top), galley, text);
            if self.menu {
                let chevron = egui::Rect::from_min_size(
                    egui::pos2(text_right + 4.0, rect.center().y - CHEVRON / 2.0),
                    vec2(CHEVRON, CHEVRON),
                );
                Icon::Down.paint(painter, chevron, text);
            }
        }
        response
    }
}
