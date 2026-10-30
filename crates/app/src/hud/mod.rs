//! The HUD: the app's main window, an opaque borderless bar on top of everything, as decision
//! 001 says: no taskbar button, cut to a rounded rectangle (Windows 10 and 11 alike), excluded
//! from capture while «Скрывать HUD из записи» is on, dragged by its grip; its place is
//! remembered per monitor.

pub mod bar;
pub mod picker;
pub mod placement;
pub mod popover;

use dc_capture::win::hud_window;
use dc_ui_kit::tokens::{BORDER, HUD_H, R3};
use eframe::egui::{self, Stroke, StrokeKind, Ui};

use crate::app::Shared;

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
    /// Size and corner radius the window was last cut to, physical pixels.
    shape: Option<(u32, u32, u32)>,
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
            shape: None,
        }
    }

    /// The bar; called every frame of the main window.
    pub fn ui(&mut self, ui: &mut Ui, shared: &mut Shared) {
        self.sync_exclusion(shared);
        let rect = ui.max_rect();
        ui.painter()
            .rect_stroke(rect, R3, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let source = bar::source_label(&shared.store.get().recording.source, &shared.sources);
        let inner = ui.horizontal_centered(|ui| bar::bar(ui, shared.store.get(), &source));
        match inner.inner {
            Some((bar::Clicked::Grip, _)) => {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            Some((bar::Clicked::More, anchor)) => toggle(shared, popover::Kind::More, anchor),
            Some((bar::Clicked::Source, anchor)) => toggle(shared, popover::Kind::Source, anchor),
            _ => {}
        }
        fit(ui, inner.response.rect.width());
        let rect = match hud_window::window_rect(self.hwnd) {
            Ok(rect) => rect,
            Err(error) => return shared.report(Err(error)),
        };
        self.round(ui, shared, rect);
        if !ui.input(|i| i.pointer.any_down()) {
            self.remember_place(shared, rect);
        }
    }

    /// Cuts the corners of the window to the bar's radius again when its size or DPI changed.
    fn round(&mut self, ui: &Ui, shared: &mut Shared, (_, _, width, height): (i32, i32, u32, u32)) {
        let radius = (f32::from(R3) * ui.ctx().pixels_per_point()).round() as u32;
        let shape = (width, height, radius);
        if self.shape != Some(shape) {
            shared.report(hud_window::round_window(self.hwnd, width, height, radius));
            self.shape = Some(shape);
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

/// Resizes the window to the bar's width.
fn fit(ui: &Ui, width: f32) {
    let size = egui::vec2(width.ceil(), HUD_H);
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
    shared.popover = (!open).then_some(popover::Open { kind, anchor });
    shared.previews = None;
    if !open && kind == popover::Kind::Source {
        shared.refresh_sources();
        let (screens, windows) = picker::cards(&shared.sources);
        let all = screens
            .iter()
            .chain(&windows)
            .map(|card| card.source)
            .collect();
        shared.previews = Some(picker::Previews::start(&shared.ctx, all));
    }
}

#[cfg(test)]
mod tests;
