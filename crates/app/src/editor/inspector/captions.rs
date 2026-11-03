//! The facts of «Субтитры» (`dc_fx_captions::panel`): the recognition of the editor
//! (`generation.rs`) in the panel's own types — the settings' types live in dc-settings, which an
//! effect may not depend on — and the cues the timeline shows. The language and the tracks the
//! panel chose go back to the generation, and from it to the settings (`window.rs`).

use dc_fx_captions::panel::{Language as Lang, List, Model, Recognition, Shown, Tracks};
use eframe::egui::Ui;

use super::Cx;
use crate::editor::generation::Generation;
use crate::editor::selection::{Item, Selected};
use crate::editor::timeline::{Row, timecode};
use crate::settings::{Language, Listen, SpeechModel};

/// The «Субтитры» tab with the facts of [`Cx`].
pub fn tab(ui: &mut Ui, cx: &mut Cx<'_>) {
    let list = list(cx);
    dc_fx_captions::panel::ui(ui, &mut cx.base, &mut cx.recognition, &list);
}

/// The cues the timeline shows (not cut out), in output order, and the one picked alone.
fn list(cx: &Cx<'_>) -> List {
    let mut shown: Vec<Shown> = cx
        .tracks
        .rows
        .iter()
        .filter(|t| t.row == Row::Captions)
        .flat_map(|t| &t.regions)
        .filter_map(|r| {
            let id = r.item.and_then(Item::id)?;
            Some(Shown {
                id,
                at: r.range,
                start: timecode(r.range.start()),
            })
        })
        .collect();
    shown.sort_by_key(|s| s.at.start());
    let picked = match cx.picked {
        Some(Selected::Region(item)) => item.id(),
        _ => None,
    };
    List { shown, picked }
}

/// The facts of the recognition; `sound` — the tracks the recordings have.
pub fn recognition(generation: &Generation, sound: &[String]) -> Recognition {
    Recognition {
        phase: generation.phase(),
        model: match generation.model {
            SpeechModel::Fast => Model::Fast,
            SpeechModel::Accurate => Model::Accurate,
        },
        model_ready: generation.model_ready(),
        language: match generation.language {
            Language::Auto => Lang::Auto,
            Language::Russian => Lang::Russian,
            Language::English => Lang::English,
        },
        tracks: match generation.listen {
            Listen::Mic => Tracks::Mic,
            Listen::System => Tracks::System,
            Listen::Both => Tracks::Both,
        },
        system: sound.iter().any(|s| s == "system"),
    }
}

/// The language and the tracks the panel chose, back into `generation`.
pub fn take(generation: &mut Generation, facts: &Recognition) {
    generation.language = match facts.language {
        Lang::Auto => Language::Auto,
        Lang::Russian => Language::Russian,
        Lang::English => Language::English,
    };
    generation.listen = match facts.tracks {
        Tracks::Mic => Listen::Mic,
        Tracks::System => Listen::System,
        Tracks::Both => Listen::Both,
    };
}
