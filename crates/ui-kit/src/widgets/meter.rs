//! Sound level indicator of the mockup (`.meter`): a 6 px track filled blue, the part past 65 %
//! yellow and past 85 % red (v1's thresholds).

use egui::{CornerRadius, Rect, Response, Sense, Ui, Widget, WidgetInfo, pos2, vec2};

use crate::tokens::{ACCENT_HI, PAUSE, REC, SURFACE_3};

/// Where the colour changes, share of the track.
const YELLOW: f32 = 0.65;
const RED: f32 = 0.85;

#[must_use = "add it with `ui.add(...)`"]
pub struct LevelBar<'a> {
    level: f32,
    width: f32,
    height: f32,
    label: &'a str,
}

impl<'a> LevelBar<'a> {
    /// `level` is the share of the track to fill, 0..1; `label` is the accessible name.
    pub fn new(level: f32, width: f32, label: &'a str) -> Self {
        Self {
            level,
            width,
            height: 6.0,
            label,
        }
    }

    /// A thinner track (the HUD's 3 px under the microphone icon).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl Widget for LevelBar<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(vec2(self.width, self.height), Sense::hover());
        let level = self.level;
        response.widget_info(|| {
            let mut info = WidgetInfo::labeled(egui::Role::ProgressIndicator, true, self.label);
            info.value = Some(f64::from(level));
            info
        });
        let radius = (self.height / 2.0) as u8;
        let painter = ui.painter();
        painter.rect_filled(rect, radius, SURFACE_3);
        let x = |share: f32| rect.left() + rect.width() * share;
        for (from, to, color) in [
            (0.0, YELLOW, ACCENT_HI),
            (YELLOW, RED, PAUSE),
            (RED, 1.0, REC),
        ] {
            if level > from {
                let part = Rect::from_min_max(
                    pos2(x(from), rect.top()),
                    pos2(x(level.min(to)), rect.bottom()),
                );
                // Only the ends of the whole fill are round: the parts join flat.
                let left = if from == 0.0 { radius } else { 0 };
                let right = if level <= to { radius } else { 0 };
                let corners = CornerRadius {
                    nw: left,
                    sw: left,
                    ne: right,
                    se: right,
                };
                painter.rect_filled(part, corners, color);
            }
        }
        response
    }
}
