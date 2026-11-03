//! Drawing over the preview frame: a picked box (mockup `.selbox`: blue outline 1.5, white
//! handles 8 with a blue border, the knob 10 on a stem 22 above the top), handles of an effect's
//! own frame and the guides (mockup `.guide`: pink for the frame, blue for another box).

use egui::{Color32, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::geom::{Boxed, Handle, KNOB, Mapping};
use super::snap::Guide;
use crate::tokens::{ACCENT_HI, GUIDE, WHITE};

/// Side of a handle, radius of the knob, points (mockup).
pub const HANDLE: f32 = 8.0;
const KNOB_RADIUS: f32 = 5.0;

/// The canvas on screen: the frame the overlay draws within.
pub fn canvas(map: Mapping) -> Rect {
    Rect::from_min_max(map.screen(Pos2::ZERO), map.screen(Pos2::ZERO + map.canvas))
}

/// The box of the picked annotation with its handles and knob.
pub fn selected(ui: &Ui, map: Mapping, boxed: &Boxed) {
    let painter = ui.painter();
    let corners: Vec<Pos2> = boxed.corners().iter().map(|p| map.screen(*p)).collect();
    let outline = Stroke::new(1.5, ACCENT_HI);
    painter.add(Shape::closed_line(corners, outline));
    let top = map.screen(boxed.handle(Handle::Top));
    let knob = map.screen(boxed.knob(map.length(KNOB)));
    painter.line_segment([top, knob], outline);
    painter.circle(knob, KNOB_RADIUS, WHITE, outline);
    for h in Handle::ALL {
        handle(ui, map.screen(boxed.handle(h)), ACCENT_HI);
    }
}

/// A white square handle with a `border` at `at`.
pub fn handle(ui: &Ui, at: Pos2, border: Color32) {
    let rect = Rect::from_center_size(at, Vec2::splat(HANDLE));
    ui.painter()
        .rect(rect, 2, WHITE, Stroke::new(1.5, border), StrokeKind::Middle);
}

/// The guides across the frame.
pub fn guides(ui: &Ui, map: Mapping, guides: &[Guide]) {
    let whole = canvas(map);
    let painter = ui.painter();
    for guide in guides {
        let colour = if guide.frame { GUIDE } else { ACCENT_HI };
        let stroke = Stroke::new(1.0, colour.gamma_multiply(0.85));
        if guide.vertical {
            let x = map.screen(pos2(guide.at, 0.0)).x;
            painter.vline(x, whole.y_range(), stroke);
        } else {
            let y = map.screen(pos2(0.0, guide.at)).y;
            painter.hline(whole.x_range(), y, stroke);
        }
    }
}
