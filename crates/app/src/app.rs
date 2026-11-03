//! The eframe app: repaints only on events (decision 002), so it uses no CPU while idle. Other
//! windows are deferred viewports that share [`Shared`] with the main one.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dc_capture::SourceList;
use dc_capture::audio::AudioSource;
use dc_ui_kit::widgets::{ToastKind, Toasts};
use eframe::egui;

use crate::editor::EditorWindow;
use crate::hotkeys::{Command, GlobalKeys};
use crate::hud::{HudWindow, picker, popover};
use crate::instance::Call;
use crate::meters::Meters;
pub use crate::monitor::on_hud_monitor;
use crate::session::Session;
use crate::settings::Hotkeys;
use crate::settings::Store;
use crate::settings_defaults::DefaultsFile;
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
    /// The picker asked for the area selection. It opens two main-window frames later, once the
    /// picker's window is gone: otherwise its pixels get into the still frame and its end
    /// takes the keyboard focus away from the new windows.
    pub area_asked: Option<crate::area::AreaAsk>,
    /// Microphone names, read when the sound popover or the settings window opens.
    pub microphones: Vec<String>,
    /// The fresh microphone list the sound popover asked for.
    pub mic_refresh: crate::microphones::Refresh,
    pub meters: Meters,
    /// The recording: countdown, recorder, timer.
    pub session: Session,
    /// The open editor.
    pub editor: Option<EditorWindow>,
    /// Why the last recording was not prepared, and its project folder.
    pub failure: Option<(String, PathBuf)>,
    /// Hotkeys whose shortcut the system or another program holds.
    pub taken_hotkeys: Vec<Command>,
    pub toasts: Toasts,
    /// The device and egui renderer the editor preview draws with; none in tests without a GPU.
    pub gpu: Option<crate::gpu::PreviewGpu>,
    /// Model downloads, shared by the settings window and the editors (`downloads.rs`).
    pub downloads: crate::downloads::Downloads,
    /// The export queue of the app, made when the first editor opens with a device.
    pub exports: Option<crate::exports::Exports>,
    /// `devcast` runs waiting for the recording (`remote.rs`).
    pub waiters: crate::remote::Waiters,
    /// Releases «Что нового» shows at the start, newest first (`hud/notices.rs`).
    pub news: Vec<dc_update::Release>,
}

/// Which windows are open and the step of the recording: what the HUD's frame acts on.
#[derive(PartialEq)]
struct Windows {
    popover: Option<popover::Open>,
    area: bool,
    settings: bool,
    editor: bool,
    session: std::mem::Discriminant<Session>,
}

impl Shared {
    fn windows(&self) -> Windows {
        Windows {
            popover: self.popover,
            area: self.area.is_some(),
            settings: self.settings.is_some(),
            editor: self.editor.is_some(),
            session: std::mem::discriminant(&self.session),
        }
    }

    /// The shared state with no sources and no microphones listed: the app lists them at its
    /// start (`lib.rs`), tests give their own.
    pub fn new(ctx: egui::Context, store: Store) -> Self {
        Self {
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
            area_asked: None,
            microphones: Vec::new(),
            mic_refresh: crate::microphones::Refresh::default(),
            meters: Meters::default(),
            session: Session::Idle,
            taken_hotkeys: Vec::new(),
            editor: None,
            failure: None,
            toasts: Toasts::default(),
            gpu: None,
            downloads: crate::downloads::Downloads::default(),
            exports: None,
            waiters: crate::remote::Waiters::default(),
            news: Vec::new(),
        }
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
        self.open_settings_at(crate::settings_window::Page::default());
    }

    /// Opens the settings window on `page`.
    pub fn open_settings_at(&mut self, page: crate::settings_window::Page) {
        self.refresh_microphones();
        let defaults = match DefaultsFile::of_app() {
            Ok(file) => file,
            Err(error) => return self.report(Err(error)),
        };
        let mut window = SettingsWindow::new(
            self.microphones.clone(),
            crate::models::Models::of_app(self.downloads.clone()),
            defaults,
        );
        window.page = page;
        self.settings = Some(window);
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
        for error in self.meters.sync(&self.ctx, &wanted) {
            self.report(Err(error));
        }
    }
}

pub struct App {
    /// Requests of the app channel: a second start, `devcast` on the editor's project.
    calls: Receiver<Call>,
    shared: Arc<Mutex<Shared>>,
    hud: HudWindow,
    tray: Tray,
    keys: GlobalKeys,
    /// `devcast app quit` runs, answered just before the app ends.
    quitting: Vec<crate::instance::Reply>,
}

impl App {
    pub fn new(
        calls: Receiver<Call>,
        shared: Shared,
        hwnd: isize,
        tray: Tray,
        keys: GlobalKeys,
    ) -> Self {
        let mut shared = shared;
        let hud = HudWindow::new(hwnd, &mut shared);
        Self {
            calls,
            shared: Arc::new(Mutex::new(shared)),
            hud,
            tray,
            keys,
            quitting: Vec::new(),
        }
    }

    /// Registers the shortcuts of the settings (none while one is being pressed in the settings,
    /// so it does not fire) and runs the pressed ones.
    fn hotkeys(&mut self) {
        let mut shared = lock(&self.shared);
        let capturing = shared
            .settings
            .as_ref()
            .is_some_and(|s| s.hotkeys.capture.is_some());
        let wanted = if capturing {
            Hotkeys {
                start_stop: None,
                pause: None,
                cancel: None,
            }
        } else {
            shared.store.get().hotkeys.clone()
        };
        if let Some(taken) = self.keys.apply(&wanted)
            && !capturing
        {
            shared.taken_hotkeys = taken;
        }
        for command in self.keys.presses() {
            shared.hotkey(command);
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

/// The shared state; a panic in another window does not lock the rest out.
pub fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A frame of a window besides the HUD. The HUD is drawn only on its own events, yet its frame
/// opens and closes the other windows and shows the settings: after a click or a key here, or
/// when this frame opened or closed a window or moved the recording on, it draws a frame too.
pub fn other_window<R>(
    ui: &mut egui::Ui,
    shared: &Mutex<Shared>,
    frame: impl FnOnce(&mut egui::Ui, &mut Shared) -> R,
) -> R {
    let mut shared = lock(shared);
    let before = shared.windows();
    let result = frame(ui, &mut shared);
    let pressed = ui.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton { .. } | egui::Event::Key { .. }
            )
        })
    });
    if pressed || shared.windows() != before {
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    result
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        crate::borderless::clear_color()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        for call in self.calls.try_iter() {
            match call.request {
                dc_ops::live::Request::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    call.reply.send(dc_ops::live::Answer::Shown);
                }
                // As the tray's «Выход» at rest: `on_exit` stops and saves a recording.
                dc_ops::live::Request::Quit => {
                    self.quitting.push(call.reply);
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                }
                request => lock(&self.shared).call(request, call.reply),
            }
        }
        lock(&self.shared).tick();
        self.hotkeys();
        self.tray();
    }

    /// A running recording is stopped and saved before the app goes.
    fn on_exit(&mut self) {
        let mut shared = lock(&self.shared);
        let recorded = std::mem::take(&mut shared.session).finish();
        if let Some(root) = &recorded {
            shared
                .waiters
                .hear(crate::remote::Heard::Stopped(root.clone()));
        }
        if let Some(Err(error)) = shared.editor.as_mut().map(|e| e.doc.flush()) {
            tracing::error!("рабочее состояние проекта не записано при выходе: {error}");
        }
        crate::editor::timeline_window::remember(&mut shared);
        for reply in self.quitting.drain(..) {
            let recorded = recorded.clone();
            reply.send_written(dc_ops::live::Answer::Quit { recorded });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut shared = lock(&self.shared);
        shared.sync_meters();
        self.hud.ui(ui, &mut shared);
        shared.toasts.show(ui.ctx());
        drop(shared);
        let bar = ui.input(|i| i.viewport().outer_rect);
        show_windows(ui.ctx(), &self.shared, bar);
    }
}

/// The windows besides the HUD, from the main window's pass; `bar` is the HUD's outer
/// rectangle. Threads (levels, previews) wake only the main window: a repaint asked of a
/// viewport that is not shown makes egui open an empty default window («egui window»), which
/// on Vulkan also crashed when it was destroyed. The main window passes the repaint on to the
/// popover and the settings, which exist for sure while it shows them.
pub fn show_windows(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, bar: Option<egui::Rect>) {
    crate::area::open_asked(ctx, shared);
    let (open, area, settings) = {
        let shared = lock(shared);
        (
            shared.popover,
            shared.area.is_some(),
            shared.settings.is_some(),
        )
    };
    if area {
        crate::area::show(ctx, shared);
    }
    crate::countdown::show(ctx, shared);
    crate::editor::show(ctx, shared);
    if let (Some(open), Some(bar)) = (open, bar) {
        popover::show(ctx, shared, open, bar);
        ctx.request_repaint_of(egui::ViewportId::from_hash_of(popover::ID));
    }
    if settings {
        settings_window::show(ctx, Arc::clone(shared));
        ctx.request_repaint_of(egui::ViewportId::from_hash_of(settings_window::ID));
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
