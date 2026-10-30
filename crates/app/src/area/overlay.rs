//! One monitor of the area selection (mockup 3): its frozen frame, the scrim outside the
//! rectangle, the blue frame with 8 handles, the size above it, the buttons under it and the
//! hint at the top. Pixels of the monitor = points × `pixels_per_point` of this window.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::area as text;
use dc_ui_kit::tokens::{ACCENT_HI, BORDER, FS_S, R1, S2, SCRIM, SURFACE, TEXT, TEXT_2, WHITE};
use dc_ui_kit::widgets::{Button, Kind, panel_frame, segmented};
use eframe::egui::{
    self, Color32, Pos2, Rect as ScreenRect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use super::model::{self, GRAB, Preset, Rect};

/// What the mouse is doing with the rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    /// A new rectangle from this corner.
    New((i32, i32)),
    /// Moving; the last pointer position.
    Move((i32, i32)),
    /// A handle: the fixed opposite point and which axes follow the pointer.
    Resize {
        anchor: (i32, i32),
        x: bool,
        y: bool,
        rect: Rect,
    },
}

/// State of the selection on this monitor.
pub struct Selection {
    pub rect: Option<Rect>,
    pub preset: Preset,
    pub drag: Option<Drag>,
    /// Monitor size, pixels.
    pub size: (i32, i32),
}

/// What the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Cancel,
    Start,
}

/// Draws the monitor and handles the mouse; the frame is `frame` (a texture of the monitor).
pub fn overlay(ui: &mut Ui, frame: Option<egui::TextureId>, sel: &mut Selection) -> Option<Action> {
    let screen = ui.max_rect();
    let ppp = ui.ctx().pixels_per_point();
    if let Some(frame) = frame {
        let uv = ScreenRect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        ui.painter().image(frame, screen, uv, Color32::WHITE);
    }
    let response = ui.interact(screen, ui.scope_id().with("area"), Sense::click_and_drag());
    let to_px = |p: Pos2| {
        (
            ((p.x - screen.left()) * ppp) as i32,
            ((p.y - screen.top()) * ppp) as i32,
        )
    };
    let to_points = |r: Rect| {
        ScreenRect::from_min_size(
            screen.min + vec2(r.x as f32, r.y as f32) / ppp,
            vec2(r.w as f32, r.h as f32) / ppp,
        )
    };
    if let Some(pointer) = response.interact_pointer_pos().map(to_px) {
        drag(sel, &response, pointer, GRAB * ppp);
    }
    match sel.rect {
        Some(rect) => paint_selection(ui, screen, to_points(rect), rect),
        None => {
            ui.painter().rect_filled(screen, 0, SCRIM);
        }
    }
    hint(ui, screen);
    let mut action = sel.rect.and_then(|rect| cluster(ui, sel, to_points(rect)));
    ui.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            action = Some(Action::Cancel);
        }
        if i.key_pressed(egui::Key::Enter) && sel.rect.is_some_and(|r| !r.too_small()) {
            action = Some(Action::Start);
        }
    });
    action
}

fn drag(sel: &mut Selection, response: &egui::Response, pointer: (i32, i32), grab: f32) {
    if response.drag_started() {
        sel.drag = Some(start(sel, pointer, grab));
    }
    let Some(drag) = sel.drag else {
        return;
    };
    let Some(next) = (match drag {
        Drag::New(anchor) => Some(model::dragged(anchor, pointer, sel.preset, sel.size)),
        Drag::Move(last) => {
            sel.drag = Some(Drag::Move(pointer));
            sel.rect
                .map(|rect| rect.moved(pointer.0 - last.0, pointer.1 - last.1, sel.size))
        }
        Drag::Resize { anchor, x, y, rect } => {
            let to = (
                if x { pointer.0 } else { rect.x + rect.w },
                if y { pointer.1 } else { rect.y + rect.h },
            );
            let anchor = (
                if x { anchor.0 } else { rect.x },
                if y { anchor.1 } else { rect.y },
            );
            Some(model::dragged(anchor, to, sel.preset, sel.size))
        }
    }) else {
        return;
    };
    sel.rect = Some(next);
    if response.drag_stopped() {
        sel.drag = None;
    }
}

/// What a press at `pointer` starts: a handle, moving, or a new rectangle.
fn start(sel: &Selection, pointer: (i32, i32), grab: f32) -> Drag {
    let Some(rect) = sel.rect else {
        return Drag::New(pointer);
    };
    let near = |a: i32, b: i32| ((a - b).abs() as f32) < grab / 2.0;
    let (left, right) = (rect.x, rect.x + rect.w);
    let (top, bottom) = (rect.y, rect.y + rect.h);
    let on_x = [
        (near(pointer.0, left), right),
        (near(pointer.0, right), left),
    ];
    let on_y = [
        (near(pointer.1, top), bottom),
        (near(pointer.1, bottom), top),
    ];
    let inside_x = pointer.0 > left && pointer.0 < right;
    let inside_y = pointer.1 > top && pointer.1 < bottom;
    let hx = on_x
        .iter()
        .find(|(hit, _)| *hit)
        .map(|(_, opposite)| *opposite);
    let hy = on_y
        .iter()
        .find(|(hit, _)| *hit)
        .map(|(_, opposite)| *opposite);
    let resizable = sel.preset != Preset::FullHd;
    match (hx, hy) {
        (Some(ax), Some(ay)) if resizable => Drag::Resize {
            anchor: (ax, ay),
            x: true,
            y: true,
            rect,
        },
        (Some(ax), None) if resizable && inside_y => Drag::Resize {
            anchor: (ax, top),
            x: true,
            y: false,
            rect,
        },
        (None, Some(ay)) if resizable && inside_x => Drag::Resize {
            anchor: (left, ay),
            x: false,
            y: true,
            rect,
        },
        _ if inside_x && inside_y => Drag::Move(pointer),
        _ if sel.preset == Preset::FullHd => Drag::Move(pointer),
        _ => Drag::New(pointer),
    }
}

fn paint_selection(ui: &Ui, screen: ScreenRect, frame: ScreenRect, rect: Rect) {
    let painter = ui.painter();
    for dim in [
        ScreenRect::from_min_max(screen.min, pos2(screen.right(), frame.top())),
        ScreenRect::from_min_max(pos2(screen.left(), frame.bottom()), screen.max),
        ScreenRect::from_min_max(
            pos2(screen.left(), frame.top()),
            pos2(frame.left(), frame.bottom()),
        ),
        ScreenRect::from_min_max(
            pos2(frame.right(), frame.top()),
            pos2(screen.right(), frame.bottom()),
        ),
    ] {
        painter.rect_filled(dim, 0, SCRIM);
    }
    painter.rect_stroke(frame, 0, Stroke::new(2.0, ACCENT_HI), StrokeKind::Outside);
    for x in [frame.left(), frame.center().x, frame.right()] {
        for y in [frame.top(), frame.center().y, frame.bottom()] {
            if (x, y) != (frame.center().x, frame.center().y) {
                let handle = ScreenRect::from_center_size(pos2(x, y), vec2(10.0, 10.0));
                painter.rect(
                    handle,
                    2,
                    WHITE,
                    Stroke::new(2.0, ACCENT_HI),
                    StrokeKind::Inside,
                );
            }
        }
    }
    let label = if rect.too_small() {
        text::too_small(rect.w.unsigned_abs(), rect.h.unsigned_abs())
    } else {
        format!("{} × {}", rect.w, rect.h)
    };
    let above = frame.top() - 30.0 > screen.top();
    let corner = if above {
        frame.left_top() - vec2(0.0, 28.0)
    } else {
        frame.left_bottom() + vec2(0.0, 6.0)
    };
    let galley = painter.layout_no_wrap(label, fonts::mono(FS_S), TEXT);
    let back = ScreenRect::from_min_size(corner, galley.size() + vec2(16.0, 6.0));
    painter.rect(
        back,
        R1,
        SURFACE,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    painter.galley(corner + vec2(8.0, 3.0), galley, TEXT);
    if !rect.too_small() {
        painter.text(
            frame.center(),
            egui::Align2::CENTER_CENTER,
            text::MOVE,
            fonts::text(FS_S, Weight::Regular),
            TEXT_2,
        );
    }
}

fn hint(ui: &Ui, screen: ScreenRect) {
    let label = format!("{} · {}", text::HINT, text::HINT_KEYS);
    let galley = ui
        .painter()
        .layout_no_wrap(label, fonts::text(FS_S, Weight::Regular), TEXT_2);
    let size = galley.size() + vec2(28.0, 8.0);
    let back = ScreenRect::from_center_size(
        pos2(screen.center().x, screen.top() + 14.0 + size.y / 2.0),
        size,
    );
    ui.painter().rect(
        back,
        size.y / 2.0,
        SURFACE,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(back.min + vec2(14.0, 4.0), galley, TEXT_2);
}

/// Presets, «Отмена» and «Начать запись» under the frame (above it near the bottom).
fn cluster(ui: &mut Ui, sel: &mut Selection, frame: ScreenRect) -> Option<Action> {
    let below = frame.bottom() + 70.0 < ui.max_rect().bottom();
    let y = if below {
        frame.bottom() + 14.0
    } else {
        frame.top() - 60.0
    };
    let mut action = None;
    egui::Area::new(ui.scope_id().with("area-buttons"))
        .fixed_pos(pos2(frame.center().x - 170.0, y))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            panel_frame().inner_margin(S2).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let before = sel.preset;
                    let full_hd = model::fits_full_hd(sel.size);
                    let options = [
                        (Preset::Free, text::FREE, true),
                        (Preset::Wide, text::WIDE, true),
                        (Preset::FullHd, text::FULL_HD, full_hd),
                    ];
                    segmented(ui, &mut sel.preset, &options);
                    if sel.preset != before {
                        sel.rect = sel
                            .rect
                            .map(|r| model::with_preset(r, sel.preset, sel.size));
                    }
                    if ui.add(Button::new(text::CANCEL).small()).clicked() {
                        action = Some(Action::Cancel);
                    }
                    let ready = sel.rect.is_some_and(|r| !r.too_small());
                    if ui
                        .add_enabled(ready, Button::new(text::START).kind(Kind::Danger).small())
                        .clicked()
                    {
                        action = Some(Action::Start);
                    }
                });
            });
        });
    action
}
