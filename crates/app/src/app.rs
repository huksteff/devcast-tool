//! The eframe app: repaints only on events (decision 002), so it uses no CPU while idle. Other
//! windows are deferred viewports that share [`Shared`] with the main one.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dc_ui_kit::strings::{hud, settings as settings_text};
use dc_ui_kit::widgets::{Button, ToastKind, Toasts};
use eframe::egui;

use crate::settings::Store;
use crate::settings_window::{self, SettingsWindow};

/// State of all windows.
pub struct Shared {
    /// The one settings store of all windows.
    pub store: Store,
    /// Open settings window.
    pub settings: Option<SettingsWindow>,
    pub toasts: Toasts,
}

impl Shared {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            settings: None,
            toasts: Toasts::default(),
        }
    }

    /// Opens the settings window with the current microphones.
    pub fn open_settings(&mut self, ctx: &egui::Context) {
        let microphones = match dc_capture::audio::list_microphones() {
            Ok(list) => list.into_iter().map(|m| m.name).collect(),
            Err(error) => {
                self.toasts.push(ctx, ToastKind::Error, error.to_string());
                Vec::new()
            }
        };
        self.settings = Some(SettingsWindow::new(microphones));
    }
}

pub struct App {
    /// A second start of the app asked to show this one.
    shown: Receiver<()>,
    shared: Arc<Mutex<Shared>>,
}

impl App {
    pub fn new(shown: Receiver<()>, store: Store) -> Self {
        Self {
            shown,
            shared: Arc::new(Mutex::new(Shared::new(store))),
        }
    }
}

/// The shared state; a panic in another window does not lock the rest out.
fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shown.try_iter().count() > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(crate::APP_NAME);
            if ui.add(Button::new(hud::SETTINGS)).clicked() {
                lock(&self.shared).open_settings(ui.ctx());
            }
        });
        lock(&self.shared).toasts.show(ui.ctx());
        if lock(&self.shared).settings.is_some() {
            show_settings(ui.ctx(), Arc::clone(&self.shared));
        }
    }
}

fn show_settings(ctx: &egui::Context, shared: Arc<Mutex<Shared>>) {
    let builder = egui::ViewportBuilder::default()
        .with_title(settings_text::TITLE)
        .with_inner_size(settings_window::SIZE)
        .with_min_inner_size(settings_window::MIN_SIZE);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("settings"),
        builder,
        move |ui, _| {
            let mut shared = lock(&shared);
            let Shared {
                store,
                settings,
                toasts,
            } = &mut *shared;
            let Some(window) = settings else {
                return;
            };
            if let Err(error) = window.ui(ui, store) {
                toasts.push(ui.ctx(), ToastKind::Error, error.to_string());
            }
            toasts.show(ui.ctx());
            if ui.input(|i| i.viewport().close_requested()) {
                *settings = None;
            }
        },
    );
}
