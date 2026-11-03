//! A check box with its caption (mockup 9, `.cbx`): a 16 × 16 box, rounded 4; blue with a white
//! tick when checked. The whole row toggles.

use egui::{Response, RichText, Sense, Ui, Widget, WidgetInfo, vec2};

use crate::icons::Icon;
use crate::tokens::{ACCENT, BORDER, FS_M, ICON, S2, SURFACE_3, TEXT, WHITE};

/// Corner radius of the box, points (mockup).
const RADIUS: f32 = 4.0;

#[must_use = "add it with `ui.add(...)`"]
pub struct Checkbox<'a> {
    on: &'a mut bool,
    label: &'a str,
}

impl<'a> Checkbox<'a> {
    pub fn new(on: &'a mut bool, label: &'a str) -> Self {
        Self { on, label }
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let galley = egui::WidgetText::from(RichText::new(self.label).size(FS_M).color(TEXT))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                ui.available_width() - ICON - S2,
                egui::TextStyle::Body,
            );
        let height = galley.size().y.max(ICON);
        let size = vec2(ICON + S2 + galley.size().x, height);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
        if response.clicked() {
            *self.on = !*self.on;
            response.mark_changed();
        }
        let on = *self.on;
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), on, self.label)
        });
        if ui.is_rect_visible(rect) {
            let box_rect = egui::Rect::from_min_size(
                rect.left_center() - vec2(0.0, ICON / 2.0),
                vec2(ICON, ICON),
            );
            let painter = ui.painter();
            if on {
                painter.rect_filled(box_rect, RADIUS, ACCENT);
                Icon::Check.paint(painter, box_rect.shrink(2.0), WHITE);
            } else {
                painter.rect_filled(box_rect, RADIUS, SURFACE_3);
                painter.rect_stroke(box_rect, RADIUS, (1.0, BORDER), egui::StrokeKind::Inside);
            }
            let text_at = egui::pos2(
                box_rect.right() + S2,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_at, galley, TEXT);
        }
        response
    }
}
