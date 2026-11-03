//! The picture of the dialog: the clip's source frame fitted into the view, its crop frame with
//! thirds and 8 handles, the rest dimmed (mockup `.cropview`); dragging the frame, an edge or a
//! handle, double click — the whole frame. The numeric fields of the frame under it.

use dc_core::PixelSize;
use dc_ui_kit::egui::{
    self, Align2, Color32, CursorIcon, DragValue, FontId, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo, pos2, vec2,
};
use dc_ui_kit::strings::crop as text;
use dc_ui_kit::tokens::{CONTROL_S, FS_S, PREVIEW_BG, R2, S2, S5, TEXT_2, TEXT_3, WHITE};

use crate::dialog::{Dialog, Picture};
use crate::dialog_geom::{Field, Frame, Handle};

/// The tallest the view gets, and the room the rest of the dialog takes in the window, points.
const MOST_HEIGHT: f32 = 460.0;
const CHROME: f32 = 260.0;
/// Side of a handle, how near a handle or an edge the pointer grabs it, points (mockup: 12).
const HANDLE: f32 = 12.0;
const GRAB: f32 = 10.0;
/// The dimmed outside of the frame (mockup: black 60 %) and the thirds (white 35 %).
const DIM: Color32 = Color32::from_black_alpha(153);
const THIRDS: Color32 = Color32::from_rgba_premultiplied(89, 89, 89, 89);
/// Width of a numeric field, points.
const FIELD: f32 = 72.0;

/// A drag of the frame: what it holds, the frame and the pointer at the press.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    handle: Handle,
    from: Frame,
    at: Pos2,
}

/// The view with the picture and the frame; handles the pointer.
pub fn picture(ui: &mut Ui, dialog: &mut Dialog, source: &mut dyn FnMut(PixelSize) -> Picture) {
    let width = ui.available_width();
    let room = ui.ctx().content_rect().height() - CHROME;
    let height = (width * 9.0 / 16.0).min(room.clamp(4.0 * S5, MOST_HEIGHT));
    let (view, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, text::FRAME));
    let area = dialog.area();
    let aspect = (area.w / area.h) as f32;
    let fitted = if view.width() / view.height() > aspect {
        vec2(view.height() * aspect, view.height())
    } else {
        vec2(view.width(), view.width() / aspect)
    };
    let shown = Rect::from_center_size(view.center(), fitted);
    let scale = shown.width() / area.w as f32;
    let on_screen = |f: &Frame| {
        Rect::from_min_size(
            shown.min + vec2(f.x as f32, f.y as f32) * scale,
            vec2(f.w as f32, f.h as f32) * scale,
        )
    };
    pointer(ui, &response, dialog, on_screen(&dialog.frame), scale);
    let size = render_size(dialog.size(), shown.size() * ui.pixels_per_point());
    let painter = ui.painter_at(view);
    painter.rect_filled(view, R2, PREVIEW_BG);
    match source(size) {
        Ok(texture) => {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(texture, shown, uv, Color32::WHITE);
        }
        Err(error) => {
            let wrap = (shown.width() - 2.0 * S5).max(1.0);
            let galley = painter.layout(error, FontId::proportional(FS_S), TEXT_2, wrap);
            let at = Align2::CENTER_CENTER.anchor_size(shown.center(), galley.size());
            painter.galley(at.min, galley, TEXT_2);
        }
    }
    // The handles stick out of the view by half their side when the frame is whole.
    let handles = ui.painter_at(view.expand(HANDLE / 2.0));
    paint_frame(&handles, shown, on_screen(&dialog.frame));
}

/// Pixels to render the source at: as many as it covers on screen, never more than it has.
fn render_size(source: PixelSize, on_screen: Vec2) -> PixelSize {
    let (w, h) = (source.width.max(1) as f32, source.height.max(1) as f32);
    let k = (on_screen.x / w).min(on_screen.y / h).min(1.0);
    let side = |s: f32| (s * k).round().max(1.0) as u32;
    PixelSize {
        width: side(w),
        height: side(h),
    }
}

/// Presses, drags and double clicks on the view; `frame` is the crop frame on screen, `scale`
/// points per px of the source.
fn pointer(ui: &Ui, response: &egui::Response, dialog: &mut Dialog, frame: Rect, scale: f32) {
    if response.double_clicked() {
        dialog.frame = Frame::full(dialog.area());
        dialog.drag = None;
        return;
    }
    let hovered = response.hover_pos().and_then(|p| grab(frame, p));
    if let Some(handle) = dialog.drag.map(|d| d.handle).or(hovered) {
        ui.ctx().set_cursor_icon(cursor(handle));
    }
    if response.drag_started()
        && let Some(at) = ui.input(|i| i.pointer.press_origin())
    {
        dialog.drag = grab(frame, at).map(|handle| Drag {
            handle,
            from: dialog.frame,
            at,
        });
    }
    if let (Some(drag), Some(now)) = (dialog.drag, response.interact_pointer_pos()) {
        let by = (now - drag.at) / scale;
        let shift = ui.input(|i| i.modifiers.shift);
        let ratio = dialog.ratio().or(shift.then(|| drag.from.ratio()));
        let by = (f64::from(by.x), f64::from(by.y));
        dialog.frame = drag.from.dragged(drag.handle, by, dialog.area(), ratio);
    }
    if response.drag_stopped() {
        dialog.drag = None;
    }
}

/// What the pointer at `p` grabs: a handle, an edge, the inside; nothing outside.
fn grab(frame: Rect, p: Pos2) -> Option<Handle> {
    let near = |handle: Handle| {
        let (ax, ay) = handle.anchor();
        let spot = frame.min + frame.size() * vec2(ax, ay);
        spot.distance(p) <= GRAB
    };
    if let Some(handle) = Handle::SIDES.into_iter().find(|h| near(*h)) {
        return Some(handle);
    }
    let across = frame.y_range().expand(GRAB).contains(p.y);
    let along = frame.x_range().expand(GRAB).contains(p.x);
    let edges = [
        (
            Handle::Left,
            across && (p.x - frame.left()).abs() <= GRAB / 2.0,
        ),
        (
            Handle::Right,
            across && (p.x - frame.right()).abs() <= GRAB / 2.0,
        ),
        (
            Handle::Top,
            along && (p.y - frame.top()).abs() <= GRAB / 2.0,
        ),
        (
            Handle::Bottom,
            along && (p.y - frame.bottom()).abs() <= GRAB / 2.0,
        ),
    ];
    if let Some((handle, _)) = edges.into_iter().find(|(_, on)| *on) {
        return Some(handle);
    }
    frame.contains(p).then_some(Handle::Move)
}

fn cursor(handle: Handle) -> CursorIcon {
    match handle {
        Handle::Move => CursorIcon::Grab,
        Handle::Left | Handle::Right => CursorIcon::ResizeHorizontal,
        Handle::Top | Handle::Bottom => CursorIcon::ResizeVertical,
        Handle::TopLeft | Handle::BottomRight => CursorIcon::ResizeNwSe,
        Handle::TopRight | Handle::BottomLeft => CursorIcon::ResizeNeSw,
    }
}

/// The dimmed outside, the white frame with its thirds and the handles.
fn paint_frame(painter: &egui::Painter, picture: Rect, frame: Rect) {
    let outside = [
        Rect::from_min_max(picture.min, pos2(picture.right(), frame.top())),
        Rect::from_min_max(pos2(picture.left(), frame.bottom()), picture.max),
        Rect::from_min_max(
            pos2(picture.left(), frame.top()),
            pos2(frame.left(), frame.bottom()),
        ),
        Rect::from_min_max(
            pos2(frame.right(), frame.top()),
            pos2(picture.right(), frame.bottom()),
        ),
    ];
    for part in outside {
        painter.rect_filled(part, 0, DIM);
    }
    for third in [1.0 / 3.0, 2.0 / 3.0] {
        let x = egui::lerp(frame.x_range(), third);
        let y = egui::lerp(frame.y_range(), third);
        painter.vline(x, frame.y_range(), Stroke::new(1.0, THIRDS));
        painter.hline(frame.x_range(), y, Stroke::new(1.0, THIRDS));
    }
    painter.rect_stroke(frame, 0, Stroke::new(2.0, WHITE), StrokeKind::Middle);
    for handle in Handle::SIDES {
        let (ax, ay) = handle.anchor();
        let spot = frame.min + frame.size() * vec2(ax, ay);
        painter.rect_filled(Rect::from_center_size(spot, Vec2::splat(HANDLE)), 2, WHITE);
    }
}

/// «X · Y · Ширина · Высота» of the frame, px of the source.
pub fn fields(ui: &mut Ui, dialog: &mut Dialog) {
    let row = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTROL_S), row, |ui| {
        ui.spacing_mut().item_spacing.x = S2;
        for (field, label) in [
            (Field::X, text::X),
            (Field::Y, text::Y),
            (Field::Width, text::WIDTH),
            (Field::Height, text::HEIGHT),
        ] {
            ui.label(RichText::new(label).size(FS_S).color(TEXT_3));
            let mut value = dialog.frame.value(field);
            let response = ui.add_sized(
                [FIELD, CONTROL_S],
                DragValue::new(&mut value)
                    .speed(1.0)
                    .max_decimals(0)
                    .update_while_editing(false),
            );
            response.widget_info(|| {
                let mut info = WidgetInfo::drag_value(true, value);
                info.label = Some(label.to_owned());
                info
            });
            if response.changed() {
                let ratio = dialog.ratio();
                dialog.frame = dialog.frame.with(field, value, dialog.area(), ratio);
            }
        }
    });
}
