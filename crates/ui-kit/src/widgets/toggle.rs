//! On/off switch of the mockup (`.toggle`): 34 × 20 track, 16 px knob; blue with a white knob
//! when on.

use egui::{Response, Sense, Ui, Widget, WidgetInfo, pos2, vec2};

use crate::tokens::{ACCENT, SURFACE_3, TEXT_2, WHITE};

const SIZE: egui::Vec2 = vec2(34.0, 20.0);
const KNOB: f32 = 8.0;

#[must_use = "add it with `ui.add(...)`"]
pub struct Toggle<'a> {
    on: &'a mut bool,
    label: &'a str,
}

impl<'a> Toggle<'a> {
    /// The size the switch takes, points.
    pub const SIZE: egui::Vec2 = SIZE;

    /// `label` is the accessible name: the visible caption is laid out by the caller.
    pub fn new(on: &'a mut bool, label: &'a str) -> Self {
        Self { on, label }
    }
}

impl Widget for Toggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, mut response) = ui.allocate_exact_size(SIZE, Sense::click());
        if response.clicked() {
            *self.on = !*self.on;
            response.mark_changed();
        }
        let on = *self.on;
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), on, self.label)
        });
        if ui.is_rect_visible(rect) {
            let t = ui.ctx().animate_bool_responsive(response.id, on);
            let painter = ui.painter();
            painter.rect_filled(
                rect,
                rect.height() / 2.0,
                if on { ACCENT } else { SURFACE_3 },
            );
            let x = egui::lerp(rect.left() + 2.0 + KNOB..=rect.right() - 2.0 - KNOB, t);
            let knob = if on { WHITE } else { TEXT_2 };
            painter.circle_filled(pos2(x, rect.center().y), KNOB, knob);
        }
        response
    }
}
