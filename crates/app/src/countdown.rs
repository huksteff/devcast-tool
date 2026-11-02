//! The countdown window (mockup 4): 200 × 200 in the middle of what will be recorded (the
//! monitor, the area or the window), the number in 88 px; a click anywhere or Esc cancels. It
//! takes the focus so Esc reaches it. It closes before the recorder starts, so it is never in
//! the recording. Its state is [`Countdown`]: the seconds left.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dc_capture::{Source, SourceList};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::countdown as text;
use dc_ui_kit::tokens::{BORDER, COUNTDOWN_FILL, FS_S, R_COUNTDOWN, S2, TEXT_3, WHITE};
use eframe::egui::{self, Rect, Sense, Ui, vec2};

use crate::app::{Shared, lock, other_window};
use crate::borderless;
use crate::session::Session;

/// Side of the panel, points; the transparent window adds the shadow margin (decision 020).
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
    let size = borderless::outer_size(vec2(SIDE, SIDE));
    let mut builder = borderless::floating(text::CANCEL_HINT)
        .with_visible(false)
        .with_resizable(false)
        .with_active(true)
        .with_inner_size(size);
    if let Some((x, y)) = center {
        let middle = egui::pos2(x as f32 / ppp, y as f32 / ppp);
        let panel = Rect::from_center_size(middle, vec2(SIDE, SIDE));
        builder = builder.with_position((panel + borderless::margin()).min);
    }
    let shared = Arc::clone(shared);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("countdown"),
        builder,
        move |ui, _| {
            other_window(ui, &shared, |ui, shared| {
                // Never in a recording, whatever moment the recorder starts at.
                if let Session::Countdown(countdown) = &mut shared.session {
                    if !countdown.excluded {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::ContentProtected(true));
                        countdown.excluded = true;
                    } else if !countdown.shown {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        countdown.shown = true;
                    }
                }
                if face(ui, left) {
                    shared.cancel_countdown();
                }
            });
        },
    );
}

/// The panel with the number and the hint, in a window with the shadow margin around it; true
/// when the user cancels (click or Esc).
pub fn face(ui: &mut Ui, left: u32) -> bool {
    let rect = borderless::panel_rect(ui.max_rect());
    let response = ui.interact(
        ui.max_rect(),
        ui.scope_id().with("countdown"),
        Sense::click(),
    );
    let painter = ui.painter();
    painter.add(borderless::panel(rect, R_COUNTDOWN, COUNTDOWN_FILL, BORDER));
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

/// One step of the countdown.
const SECOND: Duration = Duration::from_secs(1);

/// Seconds left before the recording starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Countdown {
    pub left: u32,
    /// When `left` goes down by one.
    pub next: Instant,
    /// Its window has been excluded from capture. Sent once: a command makes the window draw
    /// again, and sending it every frame drew the countdown nonstop and starved the HUD.
    pub excluded: bool,
    /// Its window is shown. It opens hidden and shows on the frame after its first: shown at
    /// once, a transparent window is black until it has drawn a picture.
    pub shown: bool,
}

impl Countdown {
    pub fn new(seconds: u32, now: Instant) -> Self {
        Self {
            left: seconds,
            next: now + SECOND,
            excluded: false,
            shown: false,
        }
    }

    /// Counts the seconds passed by `now`; true once it reached zero.
    pub fn tick(&mut self, now: Instant) -> bool {
        while self.left > 0 && now >= self.next {
            self.left -= 1;
            self.next += SECOND;
        }
        self.left == 0
    }
}
