//! The panel of a picked extra sound clip (mockup 3d, «Аудио N»): «Основное» — its file, volume,
//! gain, normalization and mute; «Место» — start on the output, the second of the file it starts
//! from and its length. Every edit is `audio set <id> …` (`devcast audio set`); «Сбросить» of
//! «Основное» brings the levels `audio add` gives (100 %, 1×, sound, no normalization).
//! Delete and «Дублировать» are in the head (`object.rs`).

use dc_core::TimeDelta;
use dc_fx_audio_tracks::{Audio, AudioClip, AudioId, MIN_LENGTH};
use dc_ui_kit::strings::{editor, inspector as names, sound as text};
use eframe::egui::{Label, RichText, Ui};
use serde_json::{Map, Value, json};

use super::sound;
use super::{Cx, Group, args, padded, reset, rows};
use crate::editor::Action;

const SET: &str = "audio set";

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, id: AudioId) {
    let Ok(state) = cx.base.project.state::<Audio>() else {
        return;
    };
    let Some(clip) = state.clips.into_iter().find(|c| c.id == id) else {
        return;
    };
    cx.base.groups.panel(sound::ID);
    let added = level_args(id, 100.0, 1.0, (false, false));
    let basic = Group {
        id: "extra",
        title: editor::BASIC,
        open: true,
        reset: Some(reset(SET, &now_args(&clip), added).into_iter().collect()),
    };
    cx.group(ui, basic, |ui, cx| level_rows(ui, cx, &clip));
    let place = Group {
        id: "extra-place",
        title: text::PLACE,
        open: true,
        reset: None,
    };
    let end = cx.tracks.duration;
    cx.group(ui, place, |ui, cx| place_rows(ui, cx, &clip, end));
    padded(ui, |ui| rows::note(ui, text::EXTRA_PLACE_NOTE));
}

fn level_rows(ui: &mut Ui, cx: &mut Cx<'_>, clip: &AudioClip) {
    let file = clip.file.path.rsplit('/').next().unwrap_or(&clip.file.path);
    rows::row(ui, text::FILE, |ui| {
        ui.add(Label::new(RichText::new(file)).truncate());
    });
    if let Some(v) = sound::volume(ui, text::VOLUME, clip.volume) {
        cx.actions.push(set(clip.id, "volume", json!(v)));
    }
    if let Some(v) = sound::gain(ui, text::GAIN, clip.gain) {
        cx.actions.push(set(clip.id, "gain", json!(v)));
    }
    if let Some(on) = rows::toggle_hint(ui, text::NORMALIZE, clip.normalize, text::LUFS) {
        cx.actions.push(set(clip.id, "normalize", json!(on)));
    }
    if let Some(on) = rows::toggle(ui, text::SILENT, clip.muted) {
        cx.actions.push(set(clip.id, "mute", json!(on)));
    }
}

/// Start, offset in the file and length, seconds, each within what `audio set` takes: the clip
/// inside the video (`end` long), not shorter than [`MIN_LENGTH`], not past the end of its file.
fn place_rows(ui: &mut Ui, cx: &mut Cx<'_>, clip: &AudioClip, end: TimeDelta) {
    let unit = (0.01, names::SECONDS);
    let (end, least) = (end.as_secs_f64(), MIN_LENGTH.as_secs_f64());
    let file = clip.file.duration.as_secs_f64();
    let start = clip.range.start().as_secs_f64();
    let length = clip.range.duration().as_secs_f64();
    let offset = clip.offset.as_secs_f64();
    let last_start = (end - least).max(0.0);
    if let Some(v) = rows::slider(ui, text::START, start, 0.0..=last_start, unit) {
        cx.actions.push(set(clip.id, "at", json!(v)));
    }
    let last_offset = (file - length).max(0.0);
    if let Some(v) = rows::slider(ui, text::OFFSET, offset, 0.0..=last_offset, unit) {
        cx.actions.push(set(clip.id, "offset", json!(v)));
    }
    let longest = (file - offset).min(end - start).max(least);
    if let Some(v) = rows::slider(ui, text::LENGTH, length, least..=longest, unit) {
        cx.actions.push(set(clip.id, "duration", json!(v)));
    }
}

/// `audio set <id> --<field> <value>`.
fn set(id: AudioId, field: &'static str, value: Value) -> Action {
    Action::op(SET, [("id", json!(id)), (field, value)])
}

fn now_args(clip: &AudioClip) -> Map<String, Value> {
    let flags = (clip.normalize, clip.muted);
    level_args(clip.id, clip.volume, clip.gain, flags)
}

/// The levels of «Основное» as arguments of `audio set`.
fn level_args(
    id: AudioId,
    volume: f64,
    gain: f64,
    (normalize, mute): (bool, bool),
) -> Map<String, Value> {
    args([
        ("id", json!(id)),
        ("volume", json!(volume)),
        ("gain", json!(gain)),
        ("normalize", json!(normalize)),
        ("mute", json!(mute)),
    ])
}
