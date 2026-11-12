//! The frame that flashes around a window picked as the source in the HUD picker, so the user
//! sees which window will be recorded: an accent outline over the window's visible frame for
//! [`SHOWN`]. A transparent, click-through window above all, excluded from capture, so it is
//! never in a recording; it shows only at rest and goes away when a countdown or a recording
//! starts.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dc_ui_kit::strings::picker as text;
use dc_ui_kit::tokens::ACCENT;
use eframe::egui::{self, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::app::{Shared, lock, other_window};
use crate::borderless;
use crate::session::Session;

/// Viewport of the frame.
pub const ID: &str = "window-flash";

/// How long the frame stays.
pub const SHOWN: Duration = Duration::from_millis(1500);

/// Width of the outline, points, drawn inside the window's frame.
const WIDTH: f32 = 3.0;

/// The frame around a picked window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFlash {
    /// Left, top, width, height of the window's visible frame, physical desktop pixels.
    frame: (i32, i32, u32, u32),
    until: Instant,
    /// Its window has been excluded from capture; sent once (see `Countdown::excluded`).
    excluded: bool,
    /// Its window is shown: it opens hidden and shows once it stands on the picked window.
    shown: bool,
}

impl WindowFlash {
    /// The frame around window `id` from `now`; none when Windows does not tell where it is.
    pub fn of(id: u64, now: Instant) -> Option<Self> {
        dc_capture::win::window_frame(id)
            .inspect_err(|e| tracing::warn!("рамки вокруг выбранного окна не будет: {e}"))
            .ok()
            .map(|frame| Self {
                frame,
                until: now + SHOWN,
                excluded: false,
                shown: false,
            })
    }
}

/// How long the frame still stays at `now`. It ends on its time and when the app leaves rest.
pub fn left(shared: &mut Shared, now: Instant) -> Option<Duration> {
    let left = shared
        .window_flash
        .filter(|_| matches!(shared.session, Session::Idle))
        .and_then(|flash| flash.until.checked_duration_since(now))
        .filter(|left| !left.is_zero());
    if left.is_none() {
        shared.window_flash = None;
    }
    left
}

/// Shows the frame while it lasts; `left` is from [`left`].
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, left: Duration) {
    let Some(frame) = lock(shared).window_flash.map(|flash| flash.frame) else {
        return;
    };
    // The main window closes the frame on its next pass after the time is up.
    ctx.request_repaint_after(left);
    let guess = target(frame, ctx.native_pixels_per_point().unwrap_or(1.0));
    let builder = borderless::floating(text::PICKED_WINDOW)
        .with_visible(false)
        .with_resizable(false)
        .with_active(false)
        .with_mouse_passthrough(true)
        .with_position(guess.min)
        .with_inner_size(guess.size());
    let shared = Arc::clone(shared);
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of(ID), builder, move |ui, _| {
        other_window(ui, &shared, |ui, shared| {
            let placed = place(ui.ctx(), frame);
            if let Some(flash) = &mut shared.window_flash {
                if !flash.excluded {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::ContentProtected(true));
                    flash.excluded = true;
                } else if !flash.shown && placed {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    flash.shown = true;
                }
            }
            let stroke = Stroke::new(WIDTH, ACCENT);
            ui.painter()
                .rect_stroke(ui.max_rect(), 0, stroke, StrokeKind::Inside);
        });
    });
}

/// Moves and sizes the window onto `frame` (physical desktop pixels); true when it stands
/// there. Set from the window's own frame in its own scale, as the countdown does: winit opens
/// a window on the primary monitor and converts points with that monitor's scale, which misses
/// a window on a monitor of another scale. Moved there, the window is rescaled by Windows, and
/// the next frame places it again.
fn place(ctx: &egui::Context, frame: (i32, i32, u32, u32)) -> bool {
    let Some(window) = ctx.input(|i| i.viewport().outer_rect) else {
        return false;
    };
    let ppp = ctx.pixels_per_point();
    if stands(window, ppp, frame) {
        return true;
    }
    let wanted = target(frame, ppp);
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(wanted.min));
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(wanted.size()));
    false
}

/// `frame` (physical desktop pixels) in points of `ppp` physical pixels.
fn target((x, y, w, h): (i32, i32, u32, u32), ppp: f32) -> Rect {
    Rect::from_min_size(pos2(x as f32, y as f32), vec2(w as f32, h as f32)) / ppp
}

/// Whether a window at `window` (points of `ppp` physical pixels) covers `frame` to a pixel.
fn stands(window: Rect, ppp: f32, frame: (i32, i32, u32, u32)) -> bool {
    let wanted = target(frame, 1.0);
    let now = window * ppp;
    let off = (now.min - wanted.min).abs().max_elem();
    let size = (now.size() - wanted.size()).abs().max_elem();
    off <= 1.0 && size <= 1.0
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use eframe::egui::{self, vec2};

    use super::{SHOWN, WindowFlash, left, stands, target};
    use crate::app::Shared;
    use crate::countdown::Countdown;
    use crate::session::Session;

    fn flashing(dir: &std::path::Path, now: Instant) -> Shared {
        let store = crate::settings::Store::load(&dir.join("settings.json")).unwrap();
        let mut shared = Shared::new(egui::Context::default(), store);
        shared.window_flash = Some(WindowFlash {
            frame: (0, 0, 800, 600),
            until: now + SHOWN,
            excluded: false,
            shown: false,
        });
        shared
    }

    /// The frame lasts its time at rest and goes with a countdown (so with a recording too).
    #[test]
    fn frame_lasts_its_time_only_at_rest() {
        let dir = tempfile::tempdir().unwrap();
        let now = Instant::now();
        let mut shared = flashing(dir.path(), now);
        assert_eq!(left(&mut shared, now), Some(SHOWN));
        let late = now + SHOWN - Duration::from_millis(100);
        assert_eq!(left(&mut shared, late), Some(Duration::from_millis(100)));
        assert_eq!(left(&mut shared, now + SHOWN), None);
        assert_eq!(shared.window_flash, None);

        let mut shared = flashing(dir.path(), now);
        shared.session = Session::Countdown(Countdown::new(3, now));
        assert_eq!(left(&mut shared, now), None);
        shared.session = Session::Idle;
        assert_eq!(left(&mut shared, now), None, "a countdown ends it for good");
    }

    /// The window covers the picked one in physical pixels at any scale of its monitor.
    #[test]
    fn frame_covers_the_window_in_physical_pixels() {
        let frame = (-1913, 207, 1280, 721);
        for ppp in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let window = target(frame, ppp);
            assert!(stands(window, ppp, frame), "{ppp}");
            assert!(
                !stands(window.translate(vec2(2.0, 0.0)), ppp, frame),
                "{ppp}"
            );
            assert!(!stands(window.expand2(vec2(0.0, 2.0)), ppp, frame), "{ppp}");
        }
    }
}
