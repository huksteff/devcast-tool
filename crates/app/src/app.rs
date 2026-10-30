//! The eframe app: repaints only on events (decision 002), so it uses no CPU while idle. Other
//! windows are deferred viewports that share [`Shared`] with the main one.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dc_capture::SourceList;
use dc_capture::audio::AudioSource;
use dc_ui_kit::strings::settings as settings_text;
use dc_ui_kit::tokens::SURFACE;
use dc_ui_kit::widgets::{ToastKind, Toasts};
use eframe::egui;

use crate::hud::{HudWindow, picker, popover};
use crate::meters::Meters;
use crate::session::Session;
use crate::settings::Store;
use crate::settings_window::{self, SettingsWindow};
use crate::tray::{Tray, TrayAction};

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
    /// Microphone names, read when the sound popover or the settings window opens.
    pub microphones: Vec<String>,
    pub meters: Meters,
    /// The recording: countdown, recorder, timer.
    pub session: Session,
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
            microphones: Vec::new(),
            meters: Meters::default(),
            session: Session::Idle,
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

    pub fn refresh_microphones(&mut self) {
        match dc_capture::audio::list_microphones() {
            Ok(list) => self.microphones = list.into_iter().map(|m| m.name).collect(),
            Err(error) => self.report(Err(error)),
        }
    }

    /// Opens the settings window with the current microphones.
    pub fn open_settings(&mut self) {
        self.refresh_microphones();
        self.settings = Some(SettingsWindow::new(self.microphones.clone()));
    }

    /// Opens the level meters the open windows show and closes the rest.
    fn sync_meters(&mut self) {
        let recording = &self.store.get().recording;
        let mut wanted = Vec::new();
        if self.popover.is_some_and(|p| p.kind == popover::Kind::Sound) {
            wanted.push(AudioSource::System);
            wanted.extend(
                self.microphones
                    .iter()
                    .cloned()
                    .map(AudioSource::Microphone),
            );
        } else if let Some(mic) = &recording.microphone
            && (self.settings.is_some() || matches!(self.session, Session::Live(_)))
        {
            wanted.push(AudioSource::Microphone(mic.clone()));
        }
        let wake = [
            egui::ViewportId::ROOT,
            egui::ViewportId::from_hash_of(popover::ID),
            egui::ViewportId::from_hash_of(SETTINGS_ID),
        ];
        for error in self.meters.sync(&self.ctx, &wake, &wanted) {
            self.report(Err(error));
        }
    }
}

/// The settings window.
const SETTINGS_ID: &str = "settings";

pub struct App {
    /// A second start of the app asked to show this one.
    shown: Receiver<()>,
    shared: Arc<Mutex<Shared>>,
    hud: HudWindow,
    tray: Tray,
}

impl App {
    pub fn new(shown: Receiver<()>, shared: Shared, hwnd: isize, tray: Tray) -> Self {
        let mut shared = shared;
        let hud = HudWindow::new(hwnd, &mut shared);
        Self {
            shown,
            shared: Arc::new(Mutex::new(shared)),
            hud,
            tray,
        }
    }

    fn tray(&mut self) {
        let mut shared = lock(&self.shared);
        for action in self.tray.actions() {
            match action {
                TrayAction::ShowHud => shared.show_hud(),
                TrayAction::Stop => shared.stop(),
                TrayAction::Settings => shared.open_settings(),
                TrayAction::Quit => shared.quit(),
            }
        }
        let view = shared.tray_view();
        let synced = self.tray.sync(view);
        shared.report(synced);
    }
}

/// Background of the HUD window. eframe wants the colour as stored in the window (sRGB bytes
/// / 255, like egui's own default), not linear: a linear #181a1f came out almost black.
pub fn clear_color() -> [f32; 4] {
    SURFACE.to_normalized_gamma_f32()
}

/// The shared state; a panic in another window does not lock the rest out.
pub fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        clear_color()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shown.try_iter().count() > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        lock(&self.shared).tick();
        self.tray();
    }

    /// A running recording is stopped and saved before the app goes.
    fn on_exit(&mut self) {
        std::mem::take(&mut lock(&self.shared).session).finish();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut shared = lock(&self.shared);
        shared.sync_meters();
        self.hud.ui(ui, &mut shared);
        shared.toasts.show(ui.ctx());
        let open = shared.popover;
        let area = shared.area.is_some();
        drop(shared);
        if area {
            crate::area::show(ui.ctx(), &self.shared);
        }
        crate::countdown::show(ui.ctx(), &self.shared);
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
        egui::ViewportId::from_hash_of(SETTINGS_ID),
        builder,
        move |ui, _| {
            let mut shared = lock(&shared);
            let Shared {
                ctx,
                store,
                settings,
                meters,
                toasts,
                ..
            } = &mut *shared;
            let Some(window) = settings else {
                return;
            };
            let level = match &store.get().recording.microphone {
                Some(mic) => meters.level(ctx, &AudioSource::Microphone(mic.clone())),
                None => 0.0,
            };
            if let Err(error) = window.ui(ui, store, level) {
                toasts.push(ui.ctx(), ToastKind::Error, error.to_string());
            }
            toasts.show(ui.ctx());
            if ui.input(|i| i.viewport().close_requested()) {
                *settings = None;
            }
        },
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn clear_color_is_the_surface_token_as_bytes() {
        let bytes = super::clear_color().map(|c| (c * 255.0).round() as u8);
        assert_eq!(
            bytes,
            [0x18, 0x1a, 0x1f, 0xff],
            "the HUD background is surface #181a1f"
        );
    }
}
