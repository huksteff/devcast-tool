//! The settings window (mockup 5): sections on the left, changes apply at once, no «Save».
//! Hotkeys get their section with the global hotkeys (6.14) and the editor shortcuts (7.3); the
//! effect defaults come from the project open in the editor (`settings_defaults.rs`, 7.30).

use dc_capture::Fps;
use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::{hud, settings as text};
use dc_ui_kit::tokens::{BG, BORDER, CONTROL, FS_S, R1, S2, S4, S5, SURFACE_2, TEXT, TEXT_2};
use dc_ui_kit::widgets::{Button, LevelBar, Toggle, dropdown, segmented};
use eframe::egui::{self, Align, Color32, Layout, Sense, Stroke, StrokeKind, Ui, vec2};

use std::sync::{Arc, Mutex};

use dc_capture::audio::AudioSource;
use dc_ui_kit::widgets::ToastKind;

use crate::app::{Shared, lock, other_window};
use crate::error::AppError;
use crate::hotkeys::Command;
use crate::models::Models;
use crate::monitor::on_hud_monitor;
use crate::settings::{AppSettings, Countdown, Store};
use crate::settings_defaults::{DefaultsFile, OpenProject};
use crate::settings_hotkeys::HotkeyPage;
use dc_ui_kit::settings_rows::{field, heading, wide_field};

/// Size of the window, points (mockup: 760 × 480, at least 640 × 420).
pub const SIZE: egui::Vec2 = vec2(760.0, 480.0);
pub const MIN_SIZE: egui::Vec2 = vec2(640.0, 420.0);
const NAV: f32 = 190.0;
/// Width of the microphone level indicator, points (mockup).
const METER: f32 = 64.0;
/// Inner side padding of the path box, points (mockup: 10 px).
const PATH_PAD: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Recording,
    Defaults,
    Hotkeys,
    Captions,
    Updates,
    About,
}

impl Page {
    const ALL: [(Self, &'static str); 6] = [
        (Self::Recording, text::RECORDING),
        (Self::Defaults, text::DEFAULTS),
        (Self::Hotkeys, text::HOTKEYS),
        (Self::Captions, text::CAPTIONS),
        (Self::Updates, text::UPDATES),
        (Self::About, text::ABOUT),
    ];
}

/// What the window keeps while open.
pub struct SettingsWindow {
    pub page: Page,
    /// Microphone names, read when the window opens.
    pub microphones: Vec<String>,
    pub hotkeys: HotkeyPage,
    /// Whisper models on disk and their downloads (cancelled when the window closes).
    pub models: Models,
    /// The effect defaults, read when the window opens.
    pub defaults: DefaultsFile,
    /// «Проверить обновления» was pressed.
    updates_checked: bool,
}

impl SettingsWindow {
    pub fn new(microphones: Vec<String>, models: Models, defaults: DefaultsFile) -> Self {
        Self {
            page: Page::default(),
            microphones,
            hotkeys: HotkeyPage::default(),
            models,
            defaults,
            updates_checked: false,
        }
    }

    /// The whole window; a failed save or model download is returned (the change is not
    /// kept). `mic_level` fills the indicator of the selected microphone, 0..1; `taken` are
    /// hotkeys the system holds; `open` is the project of the editor, if one is open.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        store: &mut Store,
        (mic_level, taken): (f32, &[Command]),
        open: Option<OpenProject<'_>>,
    ) -> Result<(), AppError> {
        let downloads = self.models.poll();
        let mut next = store.get().clone();
        egui::Panel::left("settings-nav")
            .exact_size(NAV)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG).inner_margin(S2))
            .show(ui, |ui| self.nav(ui));
        let page = egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(S5 as i8, S4 as i8)))
            .show(ui, |ui| match self.page {
                Page::Recording => {
                    self.recording(ui, &mut next, mic_level);
                    Ok(())
                }
                Page::Defaults => crate::settings_defaults::defaults(ui, &mut self.defaults, open),
                Page::Hotkeys => {
                    self.hotkeys.ui(ui, &mut next, taken);
                    Ok(())
                }
                Page::Captions => {
                    crate::settings_captions::captions(ui, &mut next, &mut self.models)
                }
                Page::Updates => {
                    self.updates(ui, &mut next);
                    Ok(())
                }
                Page::About => crate::settings_about::about(ui),
            })
            .inner;
        store.update(|settings| *settings = next)?;
        downloads.and(page)
    }

    fn nav(&mut self, ui: &mut Ui) {
        for (page, title) in Page::ALL {
            let selected = self.page == page;
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 32.0), egui::Sense::click());
            response
                .widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, title));
            if response.clicked() {
                self.page = page;
            }
            let fill = if selected || response.hovered() {
                SURFACE_2
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, R1, fill);
            let color = if selected { TEXT } else { TEXT_2 };
            ui.painter().text(
                rect.left_center() + vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                title,
                fonts::text(13.0, Weight::Regular),
                color,
            );
        }
    }

    fn recording(&mut self, ui: &mut Ui, next: &mut AppSettings, mic_level: f32) {
        heading(ui, text::RECORDING);
        field(ui, text::FOLDER, None, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let choose = Button::new(text::CHOOSE).icon(Icon::Folder);
                if ui.add(choose).clicked()
                    && let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&next.recordings_dir)
                        .pick_folder()
                {
                    next.recordings_dir = folder;
                }
                let path = next.recordings_dir.display().to_string();
                path_box(ui, &path, ui.available_width());
            });
        });
        field(ui, hud::FPS, None, |ui| fps(ui, &mut next.recording.fps));
        field(ui, hud::COUNTDOWN, None, |ui| {
            countdown(ui, &mut next.countdown)
        });
        field(ui, hud::MIC, None, |ui| {
            let mut items = vec![(None, hud::NO_MIC)];
            items.extend(
                self.microphones
                    .iter()
                    .map(|name| (Some(name.clone()), name.as_str())),
            );
            dropdown(
                ui,
                hud::MIC,
                &mut next.recording.microphone,
                &items,
                ui.available_width() - METER - ui.spacing().item_spacing.x,
            );
            // Without a microphone there is nothing to hear; its room stays so the list keeps its width.
            let listened = next.recording.microphone.is_some();
            ui.add_visible(
                listened,
                LevelBar::new(mic_level, METER, &hud::level(hud::MIC)),
            );
        });
        field(ui, hud::SYSTEM_AUDIO, None, |ui| {
            ui.add(Toggle::new(
                &mut next.recording.system_audio,
                hud::SYSTEM_AUDIO,
            ));
        });
        field(ui, hud::HIDE_FROM_RECORDING, Some(text::HIDE_HINT), |ui| {
            ui.add(Toggle::new(&mut next.hide_hud, hud::HIDE_FROM_RECORDING));
        });
    }

    fn updates(&mut self, ui: &mut Ui, next: &mut AppSettings) {
        heading(ui, text::UPDATES);
        let check = Button::new(text::CHECK_UPDATES).small();
        let status = self.updates_checked.then_some(text::MANUAL_UPDATES);
        let version = text::version(env!("CARGO_PKG_VERSION"));
        wide_field(ui, &version, status, check.size(ui).x, |ui| {
            if ui.add(check).clicked() {
                self.updates_checked = true;
            }
        });
        let toggle = Toggle::SIZE.x;
        let auto = Some(text::AUTO_UPDATES_HINT);
        wide_field(ui, text::AUTO_UPDATES, auto, toggle, |ui| {
            ui.add(Toggle::new(
                &mut next.updates.auto_check,
                text::AUTO_UPDATES,
            ));
        });
        wide_field(ui, text::BETA, None, toggle, |ui| {
            ui.add(Toggle::new(&mut next.updates.beta, text::BETA));
        });
    }
}

/// The folder path in a read-only box like an input (mockup `.path`): mono text, cut with «…»
/// at `width`.
fn path_box(ui: &mut Ui, path: &str, width: f32) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, path));
    let painter = ui.painter();
    painter.rect(
        rect,
        R1,
        SURFACE_2,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    let mut job = egui::text::LayoutJob::single_section(
        path.to_owned(),
        egui::TextFormat::simple(fonts::mono(FS_S), TEXT),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(width - 2.0 * PATH_PAD);
    let galley = painter.layout_job(job);
    let top = rect.center().y - galley.size().y / 2.0;
    painter.galley(egui::pos2(rect.left() + PATH_PAD, top), galley, TEXT);
}

/// Frame rate choice, shared with the HUD popover.
pub fn fps(ui: &mut Ui, fps: &mut Fps) {
    segmented(
        ui,
        fps,
        &[
            (Fps::Fps30, "30", true),
            (Fps::Fps60, "60", true),
            (Fps::Fps120, "120", true),
        ],
    );
}

/// Countdown choice, shared with the HUD popover.
pub fn countdown(ui: &mut Ui, countdown: &mut Countdown) {
    let labels: Vec<String> = Countdown::ALL
        .iter()
        .map(|c| match c {
            Countdown::Off => hud::COUNTDOWN_NONE.to_owned(),
            other => hud::seconds(other.seconds()),
        })
        .collect();
    let options: Vec<_> = Countdown::ALL
        .iter()
        .zip(&labels)
        .map(|(c, label)| (*c, label.as_str(), true))
        .collect();
    segmented(ui, countdown, &options);
}

/// Viewport of the settings window.
pub const ID: &str = "settings";

/// The settings window, from the main window's pass.
pub fn show(ctx: &egui::Context, shared: Arc<Mutex<Shared>>) {
    let mut builder = egui::ViewportBuilder::default()
        .with_icon(crate::logo::window_icon())
        .with_title(text::TITLE)
        .with_inner_size(SIZE)
        .with_min_inner_size(MIN_SIZE)
        .with_maximize_button(false);
    if let Some(corner) = on_hud_monitor(ctx, &lock(&shared), SIZE) {
        builder = builder.with_position(corner);
    }
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of(ID), builder, move |ui, _| {
        other_window(ui, &shared, settings_frame)
    });
}

/// One frame of the settings window.
fn settings_frame(ui: &mut egui::Ui, shared: &mut Shared) {
    crate::borderless::opaque(ui);
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
    let Some(window) = settings else {
        return;
    };
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
    if ui.input(|i| i.viewport().close_requested()) {
        *settings = None;
    }
}

#[cfg(test)]
#[path = "settings_window_tests.rs"]
mod tests;
