//! Drawing over the preview frame: the zoom frame (the rest dimmed, its scale in the corner), the
//! picked annotation's box (mockup `.selbox`: blue outline 1.5, white handles 8 with a blue
//! border, the knob 10 on a stem 22 above the top) and the guides (mockup `.guide`: pink for the
//! frame, blue for another annotation).

use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{ACCENT_HI, FS_S, GUIDE, HUE_ZOOM, WHITE};
use eframe::egui::{Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use super::geom::{Boxed, Handle, KNOB, Mapping};
use super::snap::Guide;

/// Side of a handle, radius of the knob, points (mockup).
const HANDLE: f32 = 8.0;
const KNOB_RADIUS: f32 = 5.0;
/// Outside the zoom frame: black 45 %.
const DIM: Color32 = Color32::from_black_alpha(115);

/// The canvas on screen: the frame the overlay draws within.
fn canvas(map: Mapping) -> Rect {
    Rect::from_min_max(map.screen(Pos2::ZERO), map.screen(Pos2::ZERO + map.canvas))
}

/// The zoom frame `frame` (screen) of a zoom with `scale`.
pub fn zoom(ui: &Ui, map: Mapping, frame: Rect, scale: f64) {
    let painter = ui.painter();
    let whole = canvas(map);
    let outside = [
        Rect::from_min_max(whole.min, pos2(whole.right(), frame.top())),
        Rect::from_min_max(pos2(whole.left(), frame.bottom()), whole.max),
        Rect::from_min_max(pos2(whole.left(), frame.top()), frame.left_bottom()),
        Rect::from_min_max(frame.right_top(), pos2(whole.right(), frame.bottom())),
    ];
    for part in outside.into_iter().filter(|r| r.is_positive()) {
        painter.rect_filled(part, 0, DIM);
    }
    painter.rect_stroke(
        frame,
        0,
        Stroke::new(2.0, HUE_ZOOM.line),
        StrokeKind::Middle,
    );
    for corner in [
        frame.left_top(),
        frame.right_top(),
        frame.left_bottom(),
        frame.right_bottom(),
    ] {
        handle(ui, corner, HUE_ZOOM.line);
    }
    let label = text::zoom_scale(scale);
    let galley = painter.layout_no_wrap(label, FontId::proportional(FS_S), WHITE);
    let pad = vec2(6.0, 3.0);
    let at = frame.min + vec2(HANDLE, HANDLE);
    let plate = Rect::from_min_size(at, galley.size() + 2.0 * pad);
    painter.rect_filled(plate, 4, HUE_ZOOM.fill);
    painter.galley(plate.min + pad, galley, WHITE);
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
fn handle(ui: &Ui, at: Pos2, border: Color32) {
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
