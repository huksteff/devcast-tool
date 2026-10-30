//! Text button of the mockup (`.btn`): 32 px high (small: 26), radius 6, medium 13 px text,
//! an optional icon before the text.

use egui::{Color32, Response, Sense, Stroke, StrokeKind, Ui, Widget, WidgetInfo, vec2};

use crate::fonts::{self, Weight};
use crate::icons::Icon;
use crate::tokens::{
    ACCENT, ACCENT_HI, BORDER, CONTROL, CONTROL_S, FS_M, FS_S, ICON, R1, REC, REC_STRONG, S2, S3,
    SURFACE_2, SURFACE_3, TEXT, TEXT_2, WHITE,
};

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
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            kind: Kind::Secondary,
            small: false,
            icon: None,
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
        let icon_width = self.icon.map_or(0.0, |_| ICON + 6.0);
        let width = galley.size().x + icon_width + 2.0 * padding;
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
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
                x += icon_width;
            }
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(egui::pos2(x, top), galley, text);
        }
        response
    }
}
