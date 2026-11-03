//! «Распознавание» of the captions tab and the empty card: the choices of the recognition, its
//! progress, «Создать субтитры», «Пересоздать» and «Отменить» as requests to the editor.

use dc_ui_kit::egui::{Frame, Margin, RichText, Sense, Stroke, Ui, vec2};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{Cx, Request, rows};
use dc_ui_kit::strings::{captions as text, settings, timeline};
use dc_ui_kit::tokens::{
    ACCENT_HI, BORDER, FS_M, FS_S, R2, S1, S2, S3, SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{Button, Kind, dropdown, dropdown_some};

use crate::panel::{Language, Model, Phase, Recognition, Tracks};

/// Height of the progress bar, side of the icon of the empty card, points.
const BAR: f32 = 4.0;
const BIG_ICON: f32 = 28.0;

/// The choices of the language, as in the settings window.
const LANGUAGES: [(Language, &str); 3] = [
    (Language::Auto, text::AUTO),
    (Language::Russian, text::RUSSIAN),
    (Language::English, text::ENGLISH),
];

/// The choices of the tracks, as in the settings window.
const TRACKS: [(Tracks, &str); 3] = [
    (Tracks::Mic, text::MIC),
    (Tracks::System, text::SYSTEM),
    (Tracks::Both, text::BOTH),
];

/// The group's rows; `any` — the project has cues («Пересоздать», «Очистить все»).
pub fn recognition(ui: &mut Ui, cx: &mut Cx<'_>, facts: &mut Recognition, any: bool) {
    rows::row(ui, text::LANGUAGE, |ui| {
        let width = ui.available_width();
        dropdown(ui, text::LANGUAGE, &mut facts.language, &LANGUAGES, width)
    });
    let system = facts.system;
    let tracks: Vec<_> = TRACKS
        .iter()
        .map(|&(tracks, title)| (tracks, title, system || tracks == Tracks::Mic))
        .collect();
    rows::row(ui, text::TRACKS, |ui| {
        let width = ui.available_width();
        dropdown_some(ui, text::TRACKS, &mut facts.tracks, &tracks, width)
    });
    let phase = facts.phase;
    if any {
        ui.horizontal(|ui| {
            let remake = Button::new(text::REMAKE).small();
            if ui.add_enabled(phase == Phase::Idle, remake).clicked() {
                cx.requests.push(Request::Recognize);
            }
            let clear = Button::new(text::CLEAR).kind(Kind::Danger).small();
            if ui.add(clear).clicked() {
                cx.apply("captions clear", []);
            }
        });
    }
    let model = model_name(facts.model);
    let (line, done) = match phase {
        Phase::Idle => return,
        Phase::Downloading(done) => (text::downloading(percent(done), model), done),
        Phase::Recognizing(done) => (text::recognizing(percent(done), model), done),
    };
    progress(ui, &line, done.unwrap_or(0.0));
    if ui.add(Button::new(text::CANCEL).small()).clicked() {
        cx.requests.push(Request::StopRecognizing);
    }
}

fn percent(done: Option<f32>) -> Option<u32> {
    done.map(|d| (d.clamp(0.0, 1.0) * 100.0).round() as u32)
}

fn model_name(model: Model) -> &'static str {
    match model {
        Model::Fast => settings::MODEL_FAST,
        Model::Accurate => settings::MODEL_ACCURATE,
    }
}

/// «Распознаю… 64 %» and its bar over the whole width; `done` is 0..1.
fn progress(ui: &mut Ui, line: &str, done: f32) {
    ui.label(RichText::new(line).size(FS_S).color(TEXT_2));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR), Sense::hover());
    ui.painter().rect_filled(rect, BAR / 2.0, SURFACE_3);
    let filled = rect.with_max_x(rect.left() + rect.width() * done.clamp(0.0, 1.0));
    ui.painter().rect_filled(filled, BAR / 2.0, ACCENT_HI);
}

/// «Субтитров ещё нет»: what the model costs and «Создать субтитры».
pub fn empty(ui: &mut Ui, cx: &mut Cx<'_>, facts: &Recognition) {
    let hint = if facts.model_ready {
        text::EMPTY_READY.to_owned()
    } else {
        let size = match facts.model {
            Model::Fast => text::FAST_SIZE,
            Model::Accurate => text::ACCURATE_SIZE,
        };
        text::model_missing(model_name(facts.model), size)
    };
    let idle = facts.phase == Phase::Idle;
    Frame::new()
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(R2)
        .inner_margin(Margin::same(S3 as i8))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = S1;
                let (rect, _) = ui.allocate_exact_size(vec2(BIG_ICON, BIG_ICON), Sense::hover());
                Icon::Captions.paint(ui.painter(), rect, TEXT_2);
                ui.label(RichText::new(text::EMPTY).size(FS_M).color(TEXT).strong());
                ui.label(RichText::new(hint).size(FS_S).color(TEXT_3));
                ui.add_space(S2);
                let make = Button::new(timeline::MAKE_CAPTIONS)
                    .kind(Kind::Primary)
                    .small();
                if ui.add_enabled(idle, make).clicked() {
                    cx.requests.push(Request::Recognize);
                }
            });
        });
}
