//! Square 32 px button with an icon or a short text (`.ib` of the mockup): grey, lit blue when
//! «on», a small badge in the corner (the countdown seconds).

use egui::{Color32, Rect, Response, Sense, Ui, Widget, WidgetInfo, pos2, vec2};

use crate::fonts::{self, Weight};
use crate::icons::Icon;
use crate::tokens::{
    ACCENT, ACCENT_HI, ACCENT_SOFT, CONTROL, FS_S, ICON, R1, SURFACE_3, TEXT, TEXT_2, TEXT_3,
};

/// What the button shows.
#[derive(Debug, Clone, Copy)]
enum Face<'a> {
    Icon(Icon),
    Text(&'a str),
}

#[must_use = "add it with `ui.add(...)`"]
pub struct IconButton<'a> {
    face: Face<'a>,
    label: &'a str,
    on: bool,
    dim: bool,
    badge: Option<&'a str>,
    color: Option<Color32>,
}

impl<'a> IconButton<'a> {
    /// `label` is the accessible name (the icon has no text).
    pub fn new(icon: Icon, label: &'a str) -> Self {
        Self {
            face: Face::Icon(icon),
            label,
            on: false,
            dim: false,
            badge: None,
            color: None,
        }
    }

    /// A short text instead of an icon, like the frame rate «60».
    pub fn text(text: &'a str, label: &'a str) -> Self {
        Self {
            face: Face::Text(text),
            ..Self::new(Icon::More, label)
        }
    }

    /// Lit: the thing is switched on.
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// Faded: the thing is switched off.
    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }

    pub fn badge(mut self, badge: &'a str) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Colour of the icon instead of the state colour (the red ✕ of the cancel question).
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    fn colors(&self, hovered: bool) -> (Color32, Color32) {
        let (fill, fg) = if self.on {
            (ACCENT_SOFT, ACCENT_HI)
        } else if hovered {
            (SURFACE_3, TEXT)
        } else if self.dim {
            (Color32::TRANSPARENT, TEXT_3)
        } else {
            (Color32::TRANSPARENT, TEXT_2)
        };
        (fill, self.color.unwrap_or(fg))
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(vec2(CONTROL, CONTROL), Sense::click());
        response
            .widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), self.label));
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let (fill, fg) = self.colors(response.hovered());
        let painter = ui.painter();
        painter.rect_filled(rect, R1, fill);
        match self.face {
            Face::Icon(icon) => {
                icon.paint(
                    painter,
                    Rect::from_center_size(rect.center(), vec2(ICON, ICON)),
                    fg,
                );
            }
            Face::Text(text) => {
                let font = fonts::text(FS_S, Weight::Semibold);
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, fg);
            }
        }
        if let Some(badge) = self.badge {
            paint_badge(ui, rect, badge, self.on);
        }
        response
    }
}

/// The 9 px semibold number in the lower right corner.
fn paint_badge(ui: &Ui, rect: Rect, badge: &str, on: bool) {
    let font = fonts::text(9.0, Weight::Semibold);
    let galley = ui.painter().layout_no_wrap(badge.to_owned(), font, TEXT);
    let size = galley.size() + vec2(6.0, 4.0);
    let corner = pos2(rect.right() - 1.0, rect.bottom() - 1.0);
    let badge_rect = Rect::from_min_max(corner - size, corner);
    let fill = if on { ACCENT } else { SURFACE_3 };
    ui.painter().rect_filled(badge_rect, 3, fill);
    ui.painter()
        .galley(badge_rect.min + vec2(3.0, 2.0), galley, TEXT);
}
