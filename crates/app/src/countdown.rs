//! The countdown window (mockup 4): 200 × 200 in the middle of what will be recorded (the
//! monitor, the area or the window), the number in 88 px; a click anywhere or Esc cancels. It
//! takes the focus so Esc reaches it. It closes before the recorder starts, so it is never in
//! the recording.

use std::sync::{Arc, Mutex};

use dc_capture::{Source, SourceList};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::countdown as text;
use dc_ui_kit::tokens::{BORDER, FS_S, S2, SURFACE, TEXT_3, WHITE};
use eframe::egui::{self, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::app::{Shared, lock, other_window};
use crate::session::Session;

/// Side of the window, points.
pub const SIDE: f32 = 200.0;

/// The middle of what `source` records, physical desktop pixels.
pub fn target_center(source: &Source, sources: &SourceList) -> Option<(i32, i32)> {
    let center = |x: i32, y: i32, w: u32, h: u32| (x + w as i32 / 2, y + h as i32 / 2);
    match source {
        Source::Monitor(index) => sources
            .monitors
            .iter()
            .find(|m| m.index == *index)
            .map(|m| {
                center(
                    m.bounds.x(),
                    m.bounds.y(),
                    m.bounds.width(),
                    m.bounds.height(),
                )
            }),
        Source::Area(area) => Some(center(area.x(), area.y(), area.width(), area.height())),
        Source::Window(id) => dc_capture::win::hud_window::window_rect(*id as isize)
            .inspect_err(|e| tracing::warn!("отсчёт не по центру окна: {e}"))
            .ok()
            .map(|(x, y, w, h)| center(x, y, w, h)),
    }
}

/// Shows the window while the countdown runs.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>) {
    let (left, center) = {
        let shared = lock(shared);
        let Session::Countdown(countdown) = &shared.session else {
            return;
        };
        let source = &shared.store.get().recording.source;
        (countdown.left, target_center(source, &shared.sources))
    };
    let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);
    let mut builder = egui::ViewportBuilder::default()
        .with_title(text::CANCEL_HINT)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_active(true)
        .with_inner_size(vec2(SIDE, SIDE));
    if let Some((x, y)) = center {
        builder = builder.with_position(egui::pos2(
            x as f32 / ppp - SIDE / 2.0,
            y as f32 / ppp - SIDE / 2.0,
        ));
    }
    let shared = Arc::clone(shared);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("countdown"),
        builder,
        move |ui, _| {
            // Never in a recording, whatever moment the recorder starts at; a frame a second.
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::ContentProtected(true));
            other_window(ui, &shared, |ui, shared| {
                if face(ui, left) {
                    shared.cancel_countdown();
                }
            });
        },
    );
}

/// The number and the hint; true when the user cancels (click or Esc).
pub fn face(ui: &mut Ui, left: u32) -> bool {
    let rect = ui.max_rect();
    let response = ui.interact(rect, ui.scope_id().with("countdown"), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        0,
        SURFACE,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center() - vec2(0.0, S2),
        egui::Align2::CENTER_CENTER,
        left.to_string(),
        fonts::text(88.0, Weight::Bold),
        WHITE,
    );
    let hint = Rect::from_center_size(rect.center_bottom() - vec2(0.0, 28.0), vec2(SIDE, 20.0));
    painter.text(
        hint.center(),
        egui::Align2::CENTER_CENTER,
        text::CANCEL_HINT,
        fonts::text(FS_S, Weight::Regular),
        TEXT_3,
    );
    response.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
}
