//! Slider with a number field: a 4 px rail filled blue up to a white knob, and the exact value
//! typed or dragged in a 56 px field next to it. A typed number applies on Enter or when the field
//! loses focus, not at every key.

use std::ops::RangeInclusive;

use egui::{DragValue, Rect, Response, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use crate::tokens::{ACCENT_HI, CONTROL, S2, SURFACE_3, WHITE};

const RAIL: f32 = 4.0;
const KNOB: f32 = 7.0;
const FIELD: f32 = 56.0;

#[must_use = "show it with `.show(ui)`"]
pub struct Slider<'a> {
    value: &'a mut f64,
    range: RangeInclusive<f64>,
    label: &'a str,
    width: f32,
    step: f64,
    suffix: &'a str,
    field: bool,
}

impl<'a> Slider<'a> {
    /// Width of the number field next to the rail, points.
    pub const FIELD: f32 = FIELD;

    /// `label` is the accessible name of the slider and of its field.
    pub fn new(value: &'a mut f64, range: RangeInclusive<f64>, label: &'a str) -> Self {
        Self {
            value,
            range,
            label,
            width: 160.0,
            step: 1.0,
            suffix: "",
            field: true,
        }
    }

    /// Width of the rail, px.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Values snap to multiples of `step`.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Unit after the number in the field: «%», « с».
    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    /// Only the rail, without the number field (the preview volume, the timeline scale).
    pub fn bare(mut self) -> Self {
        self.field = false;
        self
    }

    /// The rail and the field in a row; the response is changed when either changed the value.
    pub fn show(mut self, ui: &mut Ui) -> Response {
        if !self.field {
            return self.rail(ui);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            let mut rail = self.rail(ui);
            let field = ui.add_sized(
                [FIELD, CONTROL],
                DragValue::new(&mut *self.value)
                    .range(self.range.clone())
                    .speed(self.step)
                    .suffix(self.suffix)
                    .update_while_editing(false),
            );
            let value = *self.value;
            field.widget_info(|| {
                let mut info = WidgetInfo::drag_value(ui.is_enabled(), value);
                info.label = Some(self.label.to_owned());
                info
            });
            if field.changed() {
                rail.mark_changed();
            }
            rail
        })
        .inner
    }

    fn rail(&mut self, ui: &mut Ui) -> Response {
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(self.width, CONTROL), Sense::click_and_drag());
        let (min, max) = (*self.range.start(), *self.range.end());
        let left = rect.left() + KNOB;
        let right = rect.right() - KNOB;
        if let Some(pointer) = response.interact_pointer_pos() {
            let t = f64::from(((pointer.x - left) / (right - left)).clamp(0.0, 1.0));
            let value = ((min + t * (max - min)) / self.step).round() * self.step;
            if value != *self.value {
                *self.value = value;
                response.mark_changed();
            }
        }
        let value = *self.value;
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), value, self.label));
        if ui.is_rect_visible(rect) {
            let t = ((value - min) / (max - min)) as f32;
            let x = egui::lerp(left..=right, t);
            let y = rect.center().y;
            let painter = ui.painter();
            let rail = Rect::from_min_max(pos2(left, y - RAIL / 2.0), pos2(right, y + RAIL / 2.0));
            painter.rect_filled(rail, RAIL / 2.0, SURFACE_3);
            painter.rect_filled(rail.with_max_x(x), RAIL / 2.0, ACCENT_HI);
            painter.circle(pos2(x, y), KNOB, WHITE, Stroke::new(2.0, ACCENT_HI));
        }
        response
    }
}
