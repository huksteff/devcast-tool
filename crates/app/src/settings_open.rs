//! The settings window as a window: opening it (or bringing up the open one) and its frame,
//! from the main window's pass. What it shows is `settings_window.rs`.

use std::sync::{Arc, Mutex};

use dc_capture::audio::AudioSource;
use dc_ui_kit::strings::settings as text;
use dc_ui_kit::widgets::ToastKind;
use eframe::egui;

use crate::app::{Shared, lock, other_window};
use crate::borderless::show_when_drawn;
use crate::monitor::on_hud_monitor;
use crate::settings_defaults::{DefaultsFile, OpenProject};
use crate::settings_window::{MIN_SIZE, Page, SIZE, SettingsWindow, UpdateAction};

impl Shared {
    /// «Настройки»: opens the settings window with the microphones known (a fresh list
    /// follows), or brings up the open one as it is.
    pub fn open_settings(&mut self) {
        self.open_settings_at(None);
    }

    /// Opens the settings window on `page` (the first one when `None`). An open window keeps its
    /// state and only turns to `page`, if one is given; it is restored when minimized and comes
    /// to front.
    pub fn open_settings_at(&mut self, page: Option<Page>) {
        if let Some(window) = self.settings.as_mut() {
            if let Some(page) = page {
                window.page = page;
            }
            window.focus = true;
            self.ctx.send_viewport_cmd_to(
                egui::ViewportId::from_hash_of(ID),
                egui::ViewportCommand::Minimized(false),
            );
            // The main window passes the repaint on to the settings (`app::show_windows`).
            self.ctx.request_repaint_of(egui::ViewportId::ROOT);
            return;
        }
        self.fresh_microphones();
        let ctx = self.ctx.clone();
        self.mic_refresh.start(&ctx);
        let defaults = match DefaultsFile::of_app() {
            Ok(file) => file,
            Err(error) => return self.report(Err(error)),
        };
        let mut window = SettingsWindow::new(
            self.microphones.clone(),
            crate::models::Models::of_app(self.downloads.clone()),
            defaults,
        );
        window.page = page.unwrap_or_default();
        self.settings = Some(window);
    }
}

/// Viewport of the settings window.
pub const ID: &str = "settings";

/// The settings window, from the main window's pass.
pub fn show(ctx: &egui::Context, shared: Arc<Mutex<Shared>>) {
    let mut builder = egui::ViewportBuilder::default()
        .with_icon(crate::logo::window_icon())
        .with_visible(false)
        .with_title(text::TITLE)
        .with_inner_size(SIZE)
        .with_min_inner_size(MIN_SIZE)
        .with_maximize_button(false);
    if let Some(corner) = on_hud_monitor(ctx, &lock(&shared), SIZE) {
        builder = builder.with_position(corner);
    }
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of(ID), builder, move |ui, _| {
        show_when_drawn(ui.ctx());
        other_window(ui, &shared, settings_frame)
    });
}

/// One frame of the settings window.
fn settings_frame(ui: &mut egui::Ui, shared: &mut Shared) {
    crate::borderless::opaque(ui);
    if let Some(fresh) = shared.fresh_microphones().map(<[String]>::to_vec)
        && let Some(window) = shared.settings.as_mut()
    {
        window.microphones = fresh;
    }
    let update = shared.updates.state.clone();
    let Some(window) = shared.settings.as_mut() else {
        return;
    };
    window.update = update;
    // Windows gives no focus to a minimized window, and the restore lands after the command
    // that asked for it: the focus waits for a frame of the restored window.
    if window.focus && !ui.input(|i| i.viewport().minimized.unwrap_or(false)) {
        window.focus = false;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
    }
    match settings_ui(ui, shared) {
        Some(UpdateAction::Check) => shared.check_updates(),
        Some(UpdateAction::Download) => shared.download_update(),
        Some(UpdateAction::Restart) => shared.restart_into_update(),
        None => {}
    }
}

/// The window's own part of the frame; the update button pressed in it.
fn settings_ui(ui: &mut egui::Ui, shared: &mut Shared) -> Option<UpdateAction> {
    let Shared {
        ctx,
        store,
        settings,
        meters,
        toasts,
        taken_hotkeys,
        editor,
        ..
    } = shared;
    let window = settings.as_mut()?;
    let level = match &store.get().recording.microphone {
        Some(mic) => meters.level(ctx, &AudioSource::Microphone(mic.clone())),
        None => 0.0,
    };
    let open = editor.as_mut().map(|editor| OpenProject {
        name: &editor.name,
        project: editor.doc.project(),
        defaults: &mut editor.defaults,
    });
    if let Err(error) = window.ui(ui, store, (level, taken_hotkeys.as_slice()), open) {
        toasts.push(ui.ctx(), ToastKind::Error, error.to_string());
    }
    toasts.show(ui.ctx());
    let action = window.update_action.take();
    if ui.input(|i| i.viewport().close_requested()) {
        *settings = None;
    }
    action
}

#[cfg(test)]
#[path = "settings_open_tests.rs"]
mod tests;
