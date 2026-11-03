//! The HUD: the app's main window, a borderless bar on top of everything (decision 001): no
//! taskbar button, excluded from capture while «Скрывать HUD из записи» is on, dragged by its
//! grip; its place is remembered per monitor. The window is transparent and egui draws the
//! rounded bar with its border and shadow in it (decision 020).

pub mod bar;
pub mod live;
pub mod menus;
pub mod notices;
pub mod picker;
pub use dc_settings::placement;
pub mod popover;

use dc_capture::audio::AudioSource;
use dc_capture::win::hud_window;
use dc_ui_kit::tokens::{BORDER, HUD_H, PAUSE, R3, SURFACE};
use eframe::egui::{self, Ui};

use crate::borderless;

use crate::app::Shared;
use crate::session::Session;

/// Size of the bar at rest before its first layout, points (mockup: ≈ 404 × 48; ours is wider:
/// the source name «Экран 1 · 2560×1440» is shown whole).
pub const START_SIZE: egui::Vec2 = egui::vec2(480.0, HUD_H);

/// The native window of the bar.
pub struct HudWindow {
    hwnd: isize,
    /// Exclusion from capture as last set on the window.
    excluded: Option<bool>,
    /// Desktop rectangle last remembered, physical pixels.
    rect: Option<(i32, i32, u32, u32)>,
}

impl HudWindow {
    /// Puts the window where it stood last on its monitor.
    pub fn new(hwnd: isize, shared: &mut Shared) -> Self {
        let placed = hud_window::window_rect(hwnd).and_then(|(_, _, width, height)| {
            let hud = &shared.store.get().hud;
            match hud.place(&shared.sources.monitors, (width, height)) {
                Some((x, y)) => hud_window::move_window(hwnd, x, y),
                None => Ok(()),
            }
        });
        shared.report(placed);
        Self {
            hwnd,
            excluded: None,
            rect: None,
        }
    }

    /// The bar; called every frame of the main window.
    pub fn ui(&mut self, ui: &mut Ui, shared: &mut Shared) {
        self.sync_exclusion(shared);
        let paused = matches!(&shared.session, Session::Live(live) if live.paused());
        let border = if paused { PAUSE } else { BORDER };
        let window = ui.max_rect();
        let panel = borderless::panel_rect(window);
        let width = ui
            .scope_builder(egui::UiBuilder::new().max_rect(panel), |ui| {
                // The bar's width follows its content: the panel is painted under it after.
                let layer = ui.painter().add(egui::Shape::Noop);
                let width = bar_ui(ui, shared);
                let drawn = egui::Rect::from_min_size(panel.min, egui::vec2(width.ceil(), HUD_H));
                ui.painter()
                    .set(layer, borderless::panel(drawn, R3, SURFACE, border));
                width
            })
            .inner;
        fit(ui, width);
        let rect = match hud_window::window_rect(self.hwnd) {
            Ok(rect) => rect,
            Err(error) => return shared.report(Err(error)),
        };
        if !ui.input(|i| i.pointer.any_down()) {
            self.remember_place(shared, rect);
        }
    }

    fn sync_exclusion(&mut self, shared: &mut Shared) {
        let wanted = shared.store.get().hide_hud;
        if self.excluded != Some(wanted) {
            shared.report(hud_window::exclude_from_capture(self.hwnd, wanted));
            self.excluded = Some(wanted);
        }
    }

    /// Saves the place after the bar was moved.
    fn remember_place(&mut self, shared: &mut Shared, rect: (i32, i32, u32, u32)) {
        if self.rect == Some(rect) {
            return;
        }
        let first = self.rect.is_none();
        self.rect = Some(rect);
        if first {
            return;
        }
        let monitors = shared.sources.monitors.clone();
        let result = shared.store.update(|s| {
            s.hud.remember(&monitors, rect);
        });
        shared.report(result);
    }
}

/// The bar of the current state with its clicks done; returns its width. Everything of the
/// HUD but the native window, so a test drives the app through it.
pub fn bar_ui(ui: &mut Ui, shared: &mut Shared) -> f32 {
    let inner = ui.horizontal_centered(|ui| content(ui, shared));
    match inner.inner {
        Some((bar::Clicked::Grip, _)) => {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        Some((clicked, anchor)) => on_click(shared, clicked, anchor),
        None => {}
    }
    inner.response.rect.width()
}

/// The bar of the current state of the recording.
fn content(ui: &mut Ui, shared: &Shared) -> Option<(bar::Clicked, f32)> {
    let record = match &shared.session {
        Session::Live(live) => {
            let recording = &shared.store.get().recording;
            let mic = recording.microphone.as_ref().map(|name| {
                shared
                    .meters
                    .level(&shared.ctx, &AudioSource::Microphone(name.clone()))
            });
            let view = live::LiveView {
                timer: live.timer(),
                paused: live.paused(),
                mic,
                asking: matches!(
                    shared.popover,
                    Some(popover::Open {
                        kind: popover::Kind::Cancel,
                        ..
                    })
                ),
            };
            return live::live_bar(ui, &view);
        }
        Session::Preparing(_) => {
            live::preparing(ui);
            return None;
        }
        Session::Idle => bar::RecordButton::Ready,
        Session::Countdown(countdown) => bar::RecordButton::Countdown(countdown.left),
        Session::Launch | Session::Starting { .. } | Session::Cancelling(_) => {
            bar::RecordButton::Busy
        }
    };
    let settings = shared.store.get();
    let source = bar::source_label(&settings.recording.source, &shared.sources);
    bar::bar(ui, settings, &source, record)
}

fn on_click(shared: &mut Shared, clicked: bar::Clicked, anchor: f32) {
    match clicked {
        bar::Clicked::Grip => {}
        bar::Clicked::More => toggle(shared, popover::Kind::More, anchor),
        bar::Clicked::Source => toggle(shared, popover::Kind::Source, anchor),
        bar::Clicked::Mic | bar::Clicked::SystemAudio => {
            toggle(shared, popover::Kind::Sound, anchor);
        }
        bar::Clicked::Fps | bar::Clicked::Countdown => {
            toggle(shared, popover::Kind::Timing, anchor);
        }
        bar::Clicked::Record if shared.session.is_idle() => shared.record(),
        bar::Clicked::Record => shared.cancel_countdown(),
        bar::Clicked::Stop => shared.stop(),
        bar::Clicked::Pause => shared.toggle_pause(),
        bar::Clicked::Cancel => shared.ask_cancel(anchor),
        bar::Clicked::Hide => shared.hide_hud(),
    }
}

/// Resizes the window to the bar of `width` and its shadow margin.
fn fit(ui: &Ui, width: f32) {
    let size = borderless::outer_size(egui::vec2(width.ceil(), HUD_H));
    let current = ui
        .ctx()
        .input(|i| i.viewport().inner_rect.map(|r| r.size()));
    if current.is_some_and(|c| (c - size).length() > 0.5) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }
}

/// Opens the popover `kind` under the button at `anchor`, or closes it when it is open.
fn toggle(shared: &mut Shared, kind: popover::Kind, anchor: f32) {
    let open = shared.popover.is_some_and(|open| open.kind == kind);
    shared.popover = (!open).then_some(popover::Open::new(kind, anchor));
    shared.previews = None;
    if !open && kind == popover::Kind::Sound {
        // The list taken at the start or the last opening, if it came; a fresh one follows.
        if let Some(Ok(names)) = shared.mic_refresh.take() {
            shared.microphones = names;
        }
        let ctx = shared.ctx.clone();
        shared.mic_refresh.start(&ctx);
    }
    if !open && kind == popover::Kind::Source {
        shared.previews = Some(picker::Previews::start(&shared.ctx));
    }
}

#[cfg(test)]
mod tests;
