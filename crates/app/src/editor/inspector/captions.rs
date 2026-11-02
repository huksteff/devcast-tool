//! «Субтитры» (mockup 3b): the switch of the captions in the header (`captions style --show`),
//! «Распознавание» — language, tracks, «Пересоздать», «Очистить все» (`captions clear`, undone
//! by Ctrl+Z without a question, as in the mockup) and the progress with «Отменить»; «Реплики» —
//! the list (`cue.rs`), or the empty card with «Создать субтитры» while there are none; then the
//! look (`captions_style.rs`).

use dc_core::OutputTime;
use dc_fx_captions::{CaptionsEffect, CaptionsState, CueId};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{captions as text, editor, settings, timeline};
use dc_ui_kit::tokens::{
    ACCENT_HI, BORDER, FS_M, FS_S, HUE_CAPTIONS, R2, S1, S2, S3, SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{Button, Kind, Toggle, dropdown, dropdown_some};
use eframe::egui::{Frame, Margin, RichText, Sense, Stroke, Ui, vec2};
use serde_json::json;

use super::{Cx, Group, Header, captions_style, cue, header, padded, rows};
use crate::editor::Action;
use crate::editor::generation::{Generation, Phase};
use crate::settings::{Listen, SpeechModel};
use crate::settings_captions::{LANGUAGES, TRACKS};

/// Id of the panel in the remembered groups.
pub const ID: &str = "captions";
/// Height of the progress bar, side of the icon of the empty card, points.
const BAR: f32 = 4.0;
const BIG_ICON: f32 = 28.0;

/// What the captions panel works with besides the project.
pub struct Captioning<'a> {
    pub generation: &'a mut Generation,
    pub playhead: OutputTime,
    /// Sound tracks the recordings have: «Системный звук» is offered when there is one.
    pub sound: &'a [String],
    /// The cue picked alone: its line of the list opens its editor.
    pub picked: Option<CueId>,
}

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>) {
    let read = cx
        .project
        .state::<CaptionsEffect>()
        .and_then(|now| Ok((now, cx.defaults_of::<CaptionsEffect>()?.settings)));
    let (state, default) = match read {
        Ok(both) => both,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    let count = text::cues_count(state.cues.len());
    let head = Header {
        hue: HUE_CAPTIONS,
        title: editor::CAPTIONS,
        subtitle: &count,
    };
    header(ui, &head, |ui| {
        let mut show = state.settings.show;
        if ui.add(Toggle::new(&mut show, text::SHOW)).changed() {
            cx.actions.push(captions_style::set("show", json!(show)));
        }
    });
    let group = Group {
        id: "recognition",
        title: text::RECOGNITION,
        open: true,
        reset: None,
    };
    cx.group(ui, group, |ui, cx| recognition(ui, cx, &state));
    if state.cues.is_empty() {
        padded(ui, |ui| empty(ui, cx));
    } else {
        let group = Group {
            id: "cues",
            title: text::CUES,
            open: true,
            reset: None,
        };
        cx.group(ui, group, |ui, cx| cue::list(ui, cx, &state));
    }
    captions_style::groups(ui, cx, &state.settings, &default);
}

fn recognition(ui: &mut Ui, cx: &mut Cx<'_>, state: &CaptionsState) {
    let generation = &mut *cx.captioning.generation;
    rows::row(ui, text::LANGUAGE, |ui| {
        let width = ui.available_width();
        dropdown(
            ui,
            text::LANGUAGE,
            &mut generation.language,
            &LANGUAGES,
            width,
        )
    });
    let system = cx.captioning.sound.iter().any(|s| s == "system");
    let tracks: Vec<_> = TRACKS
        .iter()
        .map(|&(listen, title)| (listen, title, system || listen == Listen::Mic))
        .collect();
    rows::row(ui, text::TRACKS, |ui| {
        let width = ui.available_width();
        dropdown_some(ui, text::TRACKS, &mut generation.listen, &tracks, width)
    });
    let phase = generation.phase();
    if !state.cues.is_empty() {
        ui.horizontal(|ui| {
            let idle = phase == Phase::Idle;
            let remake = Button::new(text::REMAKE).small();
            if ui.add_enabled(idle, remake).clicked() {
                cx.actions.push(Action::Generate);
            }
            let clear = Button::new(text::CLEAR).kind(Kind::Danger).small();
            if ui.add(clear).clicked() {
                cx.actions.push(Action::op("captions clear", []));
            }
        });
    }
    let model = model_name(cx.captioning.generation.model);
    let line = match phase {
        Phase::Idle => return,
        Phase::Downloading(done) => text::downloading(percent(done), model),
        Phase::Recognizing(done) => text::recognizing(percent(done), model),
    };
    let done = match phase {
        Phase::Downloading(done) | Phase::Recognizing(done) => done.unwrap_or(0.0),
        Phase::Idle => 0.0,
    };
    progress(ui, &line, done);
    if ui.add(Button::new(text::CANCEL).small()).clicked() {
        cx.actions.push(Action::StopGenerating);
    }
}

fn percent(done: Option<f32>) -> Option<u32> {
    done.map(|d| (d.clamp(0.0, 1.0) * 100.0).round() as u32)
}

fn model_name(model: SpeechModel) -> &'static str {
    match model {
        SpeechModel::Fast => settings::MODEL_FAST,
        SpeechModel::Accurate => settings::MODEL_ACCURATE,
    }
}

/// «Распознаю… 64 %» and its bar over the whole width.
fn progress(ui: &mut Ui, line: &str, done: f32) {
    ui.label(RichText::new(line).size(FS_S).color(TEXT_2));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR), Sense::hover());
    ui.painter().rect_filled(rect, BAR / 2.0, SURFACE_3);
    let filled = rect.with_max_x(rect.left() + rect.width() * done.clamp(0.0, 1.0));
    ui.painter().rect_filled(filled, BAR / 2.0, ACCENT_HI);
}

/// «Субтитров ещё нет»: what the model costs and «Создать субтитры».
fn empty(ui: &mut Ui, cx: &mut Cx<'_>) {
    let generation = &cx.captioning.generation;
    let hint = if generation.model_ready() {
        text::EMPTY_READY.to_owned()
    } else {
        let size = match generation.model {
            SpeechModel::Fast => text::FAST_SIZE,
            SpeechModel::Accurate => text::ACCURATE_SIZE,
        };
        text::model_missing(model_name(generation.model), size)
    };
    let idle = generation.phase() == Phase::Idle;
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
                    cx.actions.push(Action::Generate);
                }
            });
        });
}
