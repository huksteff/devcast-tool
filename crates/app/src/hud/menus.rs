//! The sound and timing popovers of the HUD (mockup 1): «Звук» with the system sound and the
//! microphones, each with its live level; «Частота кадров» and «Обратный отсчёт». They change
//! the same settings as the settings window.

use dc_capture::RecordingConfig;
use dc_capture::audio::AudioSource;
use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::hud;
use dc_ui_kit::tokens::{
    ACCENT_HI, ACCENT_SOFT, FS_M, FS_S, ICON, R1, S2, SURFACE_2, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{LevelBar, Toggle};
use eframe::egui::{self, Color32, Sense, Ui, WidgetInfo, pos2, vec2};

use super::popover::divider;
use crate::settings::AppSettings;
use crate::settings_window;

/// Width of the sound popover, points (mockup `.pop`).
pub const SOUND_WIDTH: f32 = 300.0;
/// Width of the timing popover, points.
pub const TIMING_WIDTH: f32 = 220.0;

const ROW: f32 = 34.0;
const METER: f32 = 64.0;

/// «Звук» and «Микрофон»: the system sound switch and the microphone list. A click on a row
/// picks the microphone and keeps the popover open (v1).
pub fn sound_menu(
    ui: &mut Ui,
    recording: &mut RecordingConfig,
    microphones: &[String],
    level: &dyn Fn(&AudioSource) -> f32,
) {
    ui.spacing_mut().item_spacing.y = 2.0;
    heading(ui, hud::SOUND);
    ui.horizontal(|ui| {
        ui.set_min_height(ROW);
        ui.add_space(S2);
        icon(ui, Icon::Speaker, TEXT);
        let gaps = 2.0 * ui.spacing().item_spacing.x + S2;
        name(
            ui,
            hud::SYSTEM_AUDIO,
            TEXT,
            ui.available_width() - 56.0 - 34.0 - gaps,
        );
        ui.add(LevelBar::new(
            level(&AudioSource::System),
            56.0,
            &hud::level(hud::SYSTEM_AUDIO),
        ));
        ui.add(Toggle::new(&mut recording.system_audio, hud::SYSTEM_AUDIO));
    });
    divider(ui);
    heading(ui, hud::MIC);
    if row(ui, recording.microphone.is_none(), hud::NO_MIC, None).clicked() {
        recording.microphone = None;
    }
    if microphones.is_empty() {
        ui.label(egui::RichText::new(hud::NO_MICS).color(TEXT_3).size(FS_S));
    }
    for mic in microphones {
        let chosen = recording.microphone.as_ref() == Some(mic);
        let source = AudioSource::Microphone(mic.clone());
        if row(ui, chosen, mic, Some(level(&source))).clicked() {
            recording.microphone = Some(mic.clone());
        }
    }
}

/// «Частота кадров» and «Обратный отсчёт», as in the settings window.
pub fn timing_menu(ui: &mut Ui, settings: &mut AppSettings) {
    heading(ui, hud::FPS);
    ui.horizontal(|ui| {
        ui.add_space(S2);
        settings_window::fps(ui, &mut settings.recording.fps);
    });
    divider(ui);
    heading(ui, hud::COUNTDOWN);
    ui.horizontal(|ui| {
        ui.add_space(S2);
        settings_window::countdown(ui, &mut settings.countdown);
    });
}

/// A grey caption in capitals (`.pop h4`).
fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(S2);
        ui.label(
            egui::RichText::new(text.to_uppercase())
                .font(fonts::text(FS_S, Weight::Semibold))
                .color(TEXT_3),
        );
    });
    ui.add_space(4.0);
}

/// A microphone row: check mark, name, level; «Без микрофона» has the crossed icon instead.
fn row(ui: &mut Ui, chosen: bool, text: &str, level: Option<f32>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::Role::MenuItem, true, chosen, text));
    let fill = if chosen {
        ACCENT_SOFT
    } else if response.hovered() {
        SURFACE_2
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, R1, fill);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add_space(S2);
    match level {
        None => icon(&mut child, Icon::MicOff, TEXT_3),
        Some(_) if chosen => icon(&mut child, Icon::Check, ACCENT_HI),
        Some(_) => {
            child.add_space(ICON);
        }
    }
    let color = if level.is_none() { TEXT_2 } else { TEXT };
    let meter = if level.is_some() {
        METER + child.spacing().item_spacing.x
    } else {
        0.0
    };
    let width = child.available_width() - child.spacing().item_spacing.x - meter - S2;
    name(&mut child, text, color, width);
    if let Some(level) = level {
        child.add(LevelBar::new(level, METER, &hud::level(text)));
    }
    response
}

fn icon(ui: &mut Ui, icon: Icon, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
    icon.paint(ui.painter(), rect, color);
}

/// Text cut with «…» to `width`.
fn name(ui: &mut Ui, text: &str, color: Color32, width: f32) {
    let format = egui::TextFormat::simple(fonts::text(FS_M, Weight::Regular), color);
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), format);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    let galley = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
    ui.painter().galley(
        pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}
