//! «Субтитры» (mockup 3b), the rail tab ([`ui`]): the switch of the captions in the header
//! (`captions style --show`), «Распознавание» — language, tracks, «Пересоздать», «Очистить все»
//! (`captions clear`, undone by Ctrl+Z without a question, as in the mockup) and the progress
//! with «Отменить» (`panel_recognition.rs`); «Реплики» — the list (`panel_cue.rs`), or the empty
//! card with «Создать субтитры» while there are none; then the look (`panel_style.rs`). A cue
//! picked on the timeline gets its own panel ([`cue`]).
//!
//! Recognition runs in the editor (a worker process, decision 018): the panel gets its state as
//! [`Recognition`] and asks to start or stop it with `Request::Recognize`/`StopRecognizing`.
//! The language and the tracks are settings of the app, not of the project, so the panel changes
//! them in the facts and the editor writes them back to the settings after the layout.

use dc_core::time::Output;
use dc_core::{Effect, Range};
use dc_ui_kit::egui::Ui;
use dc_ui_kit::panel::{Cx, Group, Header, header, padded, rows};
use dc_ui_kit::strings::{captions as text, editor};
use dc_ui_kit::tokens::HUE_CAPTIONS;
use dc_ui_kit::widgets::Toggle;
use serde_json::json;

pub use crate::panel_cue::cue;
use crate::panel_recognition::{empty, recognition};
use crate::panel_style as style;
use crate::{CaptionsEffect, CueId};

/// Id of the captions panels in the remembered groups.
pub const ID: &str = CaptionsEffect::ID;

/// What the panel shows of the recognition; the editor fills it every frame and takes
/// [`Recognition::language`] and [`Recognition::tracks`] back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recognition {
    pub phase: Phase,
    /// The model of the settings: its name on the progress line and its size on the empty card.
    pub model: Model,
    /// The model is on disk: the empty card does not offer to download it.
    pub model_ready: bool,
    /// The language Whisper listens for; the panel's list changes it.
    pub language: Language,
    /// The tracks of the recordings to listen to; the panel's list changes it.
    pub tracks: Tracks,
    /// The recordings have system sound: «Системный звук» and «Оба» can be chosen.
    pub system: bool,
}

/// Where the recognition is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Idle,
    /// The model is being downloaded: its share 0..1, once the size is known.
    Downloading(Option<f32>),
    /// The worker recognizes: its share 0..1, once it said.
    Recognizing(Option<f32>),
}

/// The speech model of the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Fast,
    Accurate,
}

/// The language of the recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Auto,
    Russian,
    English,
}

/// The tracks of the recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tracks {
    Mic,
    System,
    /// Both; the microphone wins where they overlap.
    Both,
}

/// The cues of the list as the timeline shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct List {
    /// The cues not cut out, in output order.
    pub shown: Vec<Shown>,
    /// The cue picked alone: its line opens its editor under it.
    pub picked: Option<CueId>,
}

/// A cue on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub id: CueId,
    /// Where it is in the output.
    pub at: Range<Output>,
    /// Its start as the timeline writes it («0:09.00»).
    pub start: String,
}

/// The «Субтитры» tab.
pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, recognizing: &mut Recognition, list: &List) {
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
            cx.apply(style::OP, [("show", json!(show))]);
        }
    });
    let group = Group {
        id: "recognition",
        title: text::RECOGNITION,
        open: true,
        reset: None,
    };
    let any = !state.cues.is_empty();
    cx.group(ui, group, |ui, cx| recognition(ui, cx, recognizing, any));
    if any {
        let group = Group {
            id: "cues",
            title: text::CUES,
            open: true,
            reset: None,
        };
        cx.group(ui, group, |ui, cx| {
            crate::panel_cue::list(ui, cx, &state, list)
        });
    } else {
        padded(ui, |ui| empty(ui, cx, recognizing));
    }
    style::groups(ui, cx, &state.settings, &default);
}
