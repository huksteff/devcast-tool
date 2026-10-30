//! The settings window (mockup 5): sections on the left, changes apply at once, no «Save».
//! Hotkeys get their section with the global hotkeys (6.14); effect defaults are changed in the
//! editor (stage 7).

use dc_capture::Fps;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::{hud, settings as text};
use dc_ui_kit::tokens::{
    BG, BORDER, FS_L, FS_S, R1, S2, S3, S4, S5, SURFACE_2, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{Button, LevelBar, Toggle, dropdown, segmented};
use eframe::egui::{self, Color32, RichText, Stroke, Ui, vec2};

use crate::error::AppError;
use crate::settings::{AppSettings, Countdown, SpeechModel, Store};

/// Size of the window, points (mockup: 760 × 480, at least 640 × 420).
pub const SIZE: egui::Vec2 = vec2(760.0, 480.0);
pub const MIN_SIZE: egui::Vec2 = vec2(640.0, 420.0);
const NAV: f32 = 190.0;
const LABEL: f32 = 190.0;
/// Width of the microphone level indicator, points (mockup).
const METER: f32 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Recording,
    Captions,
    Updates,
    About,
}

impl Page {
    const ALL: [(Self, &'static str); 4] = [
        (Self::Recording, text::RECORDING),
        (Self::Captions, text::CAPTIONS),
        (Self::Updates, text::UPDATES),
        (Self::About, text::ABOUT),
    ];
}

/// What the window keeps while open.
#[derive(Debug, Default)]
pub struct SettingsWindow {
    pub page: Page,
    /// Microphone names, read when the window opens.
    pub microphones: Vec<String>,
    /// «Проверить обновления» was pressed.
    updates_checked: bool,
}

impl SettingsWindow {
    pub fn new(microphones: Vec<String>) -> Self {
        Self {
            microphones,
            ..Self::default()
        }
    }

    /// The whole window; a failed save is returned (the change is not kept). `mic_level` fills
    /// the indicator of the selected microphone, 0..1.
    pub fn ui(&mut self, ui: &mut Ui, store: &mut Store, mic_level: f32) -> Result<(), AppError> {
        let mut next = store.get().clone();
        egui::Panel::left("settings-nav")
            .exact_size(NAV)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(S2)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| self.nav(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(S5 as i8, S4 as i8)))
            .show(ui, |ui| match self.page {
                Page::Recording => self.recording(ui, &mut next, mic_level),
                Page::Captions => captions(ui, &mut next),
                Page::Updates => self.updates(ui, &mut next),
                Page::About => about(ui),
            });
        store.update(|settings| *settings = next)
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
            let path = next.recordings_dir.display().to_string();
            ui.label(RichText::new(path).font(fonts::mono(FS_S)).color(TEXT));
            if ui.add(Button::new(text::CHOOSE).small()).clicked()
                && let Some(folder) = rfd::FileDialog::new()
                    .set_directory(&next.recordings_dir)
                    .pick_folder()
            {
                next.recordings_dir = folder;
            }
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
                ui.available_width() - S3 - METER - S2,
            );
            ui.add(LevelBar::new(mic_level, METER, &hud::level(hud::MIC)));
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
        field(ui, &text::version(env!("CARGO_PKG_VERSION")), None, |ui| {
            if ui.add(Button::new(text::CHECK_UPDATES).small()).clicked() {
                self.updates_checked = true;
            }
            if self.updates_checked {
                ui.label(RichText::new(text::MANUAL_UPDATES).color(TEXT_3).size(FS_S));
            }
        });
        field(
            ui,
            text::AUTO_UPDATES,
            Some(text::AUTO_UPDATES_HINT),
            |ui| {
                ui.add(Toggle::new(
                    &mut next.updates.auto_check,
                    text::AUTO_UPDATES,
                ));
            },
        );
        field(ui, text::BETA, None, |ui| {
            ui.add(Toggle::new(&mut next.updates.beta, text::BETA));
        });
    }
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

fn captions(ui: &mut Ui, next: &mut AppSettings) {
    heading(ui, text::CAPTIONS);
    ui.label(RichText::new(text::MODEL).color(TEXT_2));
    ui.add_space(S2);
    ui.horizontal(|ui| {
        for (model, title, hint) in [
            (SpeechModel::Fast, text::MODEL_FAST, text::MODEL_FAST_HINT),
            (
                SpeechModel::Accurate,
                text::MODEL_ACCURATE,
                text::MODEL_ACCURATE_HINT,
            ),
        ] {
            let selected = next.speech_model == model;
            let response = ui.radio(selected, format!("{title}\n{hint}"));
            if response.clicked() {
                next.speech_model = model;
            }
        }
    });
}

fn about(ui: &mut Ui) {
    heading(ui, text::ABOUT);
    ui.label(RichText::new(text::version(env!("CARGO_PKG_VERSION"))).color(TEXT_2));
}

fn heading(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .font(fonts::text(FS_L, Weight::Semibold))
            .color(TEXT),
    );
    ui.add_space(S3);
}

/// A row: caption (and a grey hint under it) on the left, the control on the right.
fn field(ui: &mut Ui, label: &str, hint: Option<&str>, value: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(48.0);
        ui.allocate_ui(vec2(LABEL, 40.0), |ui| {
            ui.set_width(LABEL);
            ui.vertical(|ui| {
                ui.label(RichText::new(label).color(TEXT_2));
                if let Some(hint) = hint {
                    ui.label(RichText::new(hint).color(TEXT_3).size(11.0));
                }
            });
        });
        ui.add_space(S3);
        value(ui);
    });
    let y = ui.cursor().top();
    let x = ui.max_rect().x_range();
    ui.painter().hline(x, y, Stroke::new(1.0, SURFACE_2));
}

#[cfg(test)]
#[path = "settings_window_tests.rs"]
mod tests;
