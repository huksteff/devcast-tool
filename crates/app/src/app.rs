//! The eframe app: repaints only on events (decision 002), so it uses no CPU while idle. Other
//! windows are deferred viewports that share [`Shared`] with the main one.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dc_capture::SourceList;
use dc_ui_kit::strings::settings as settings_text;
use dc_ui_kit::tokens::SURFACE;
use dc_ui_kit::widgets::{ToastKind, Toasts};
use eframe::egui;

use crate::hud::{HudWindow, picker, popover};
use crate::settings::Store;
use crate::settings_window::{self, SettingsWindow};

/// State of all windows.
pub struct Shared {
    pub ctx: egui::Context,
    /// The one settings store of all windows.
    pub store: Store,
    /// Monitors and windows, read at the start and when the source picker opens.
    pub sources: SourceList,
    /// Open settings window.
    pub settings: Option<SettingsWindow>,
    /// Open popover of the HUD.
    pub popover: Option<popover::Open>,
    /// Previews of the open source picker.
    pub previews: Option<picker::Previews>,
    /// Open area selection.
    pub area: Option<crate::area::AreaSession>,
    pub toasts: Toasts,
}

impl Shared {
    pub fn new(ctx: egui::Context, store: Store) -> Self {
        let mut shared = Self {
            ctx,
            store,
            sources: SourceList {
                monitors: Vec::new(),
                windows: Vec::new(),
            },
            settings: None,
            popover: None,
            previews: None,
            area: None,
            toasts: Toasts::default(),
        };
        shared.refresh_sources();
        shared
    }

    pub fn refresh_sources(&mut self) {
        match dc_capture::win::list_sources() {
            Ok(sources) => self.sources = sources,
            Err(error) => self.report(Err(error)),
        }
    }

    /// An error of an action goes to a toast (and the log).
    pub fn report<E: std::fmt::Display>(&mut self, result: Result<(), E>) {
        if let Err(error) = result {
            tracing::warn!("{error}");
            self.toasts
                .push(&self.ctx, ToastKind::Error, error.to_string());
        }
    }

    /// Opens the settings window with the current microphones.
    pub fn open_settings(&mut self) {
        let microphones = match dc_capture::audio::list_microphones() {
            Ok(list) => list.into_iter().map(|m| m.name).collect(),
            Err(error) => {
                self.report(Err(error));
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
    hud: HudWindow,
}

impl App {
    pub fn new(shown: Receiver<()>, shared: Shared, hwnd: isize) -> Self {
        let mut shared = shared;
        let hud = HudWindow::new(hwnd, &mut shared);
        Self {
            shown,
            shared: Arc::new(Mutex::new(shared)),
            hud,
        }
    }
}

/// The shared state; a panic in another window does not lock the rest out.
pub fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(SURFACE).to_array()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shown.try_iter().count() > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut shared = lock(&self.shared);
        self.hud.ui(ui, &mut shared);
        shared.toasts.show(ui.ctx());
        let open = shared.popover;
        let area = shared.area.is_some();
        drop(shared);
        if area {
            crate::area::show(ui.ctx(), &self.shared);
        }
        let bar = ui.input(|i| i.viewport().outer_rect);
        if let (Some(open), Some(bar)) = (open, bar) {
            popover::show(ui.ctx(), &self.shared, open, bar);
        }
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
                ..
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
