//! «Звук» (mockup 3d): the default levels of the recordings' sound tracks — a group per track the
//! recordings have (microphone, system): volume, gain, normalization, noise suppression (only the
//! microphone, owner's decision 2026-10-31), and the mute in the group's head. Every edit is
//! `audio channel <track> …` (`devcast audio channel`), «Сбросить» brings the track to the
//! user's defaults with one such command. A clip with a level of its own keeps it (decision
//! 000). Extra sound has no track levels: each clip has its own ([`extra`]); the sound of a clip
//! is [`clip`].

use dc_core::OpName;
use std::collections::BTreeMap;

use dc_core::{ChannelLevels, ClipId, TimeDelta};
use dc_ui_kit::egui::{Rect, Ui, UiBuilder};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{BUTTON, CHEVRON, Cx, Group, Header, args, header, padded, reset, rows};
use dc_ui_kit::strings::{inspector as names, sound as text};
use dc_ui_kit::tokens::{self, HUE_AUDIO};
use dc_ui_kit::widgets::IconButton;
use serde_json::{Map, Value, json};

use crate::{Audio, DENOISED, MIC_TRACK, SYSTEM_TRACK};

pub use crate::panel_clip::clip;
pub use crate::panel_extra::extra;

/// Id of the sound panels in the remembered groups (`sound.mic`; older than [`Audio`]'s id).
pub const ID: &str = "sound";
const OP: OpName = OpName::of::<crate::AudioChannel>();

/// The recordings' tracks the panels show, in the order of the timeline rows.
const RECORDED: [(&str, &str); 2] = [(MIC_TRACK, text::MIC), (SYSTEM_TRACK, text::SYSTEM)];

/// What only the editor knows of the sound, read from the files: it fills it for the panels.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    /// Sound tracks the recordings of the project have ([`MIC_TRACK`], [`SYSTEM_TRACK`]): the
    /// groups of the panels.
    pub recorded: &'a [String],
    /// Sound tracks of the imported video of the clip the panel shows, as they come
    /// (`audio-1`, `audio-2` …); `None` until they are read, and for a recording.
    pub streams: Option<&'a [String]>,
    /// Length of the finished video: an extra sound stays inside it.
    pub end: TimeDelta,
}

/// The tracks of [`RECORDED`] the recordings of the project have: (name, title).
pub(crate) fn recorded(sound: &[String]) -> impl Iterator<Item = (&'static str, &'static str)> {
    RECORDED
        .into_iter()
        .filter(|(name, _)| sound.iter().any(|s| s == name))
}

/// Levels of a track with the neutral ones where none is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Levels {
    /// Percent, 0..100.
    pub volume: f64,
    /// Times, 1..10.
    pub gain: f64,
    pub muted: bool,
    pub normalize: bool,
    pub denoise: bool,
}

impl Levels {
    pub fn of(levels: &ChannelLevels) -> Self {
        Self {
            volume: levels.volume.unwrap_or(100.0),
            gain: levels.gain.unwrap_or(1.0),
            muted: levels.muted.unwrap_or(false),
            normalize: levels.normalize.unwrap_or(false),
            denoise: levels.denoise.unwrap_or(false),
        }
    }
}

/// The «Звук» tab: its edits go to `cx.edits`.
pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, sources: &Sources<'_>) {
    let head = Header {
        hue: HUE_AUDIO,
        title: names::SOUND,
        subtitle: text::DEFAULTS_FOR_ALL,
    };
    header(ui, &head, |_| {});
    let read = cx
        .project
        .state::<Audio>()
        .and_then(|now| Ok((now.settings, cx.defaults_of::<Audio>()?.settings)));
    let (now, default) = match read {
        Ok(both) => both,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    let tracks: Vec<_> = recorded(sources.recorded).collect();
    if tracks.is_empty() {
        padded(ui, |ui| rows::note(ui, text::NO_SOUND));
    }
    for (name, title) in tracks {
        let level = |channels: &BTreeMap<String, ChannelLevels>| {
            Levels::of(&channels.get(name).cloned().unwrap_or_default())
        };
        track(
            ui,
            cx,
            (name, title),
            level(&now.channels),
            level(&default.channels),
        );
    }
    padded(ui, |ui| {
        rows::note(ui, text::UNCHANGED_NOTE);
        rows::note(ui, text::EXTRA_NOTE);
    });
}

/// The group of track `name`: its mute in the head, its rows when open.
fn track(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    (name, title): (&'static str, &str),
    now: Levels,
    default: Levels,
) {
    let group = Group {
        id: name,
        title,
        open: true,
        reset: Some(
            reset(OP, &all_args(name, &now), all_args(name, &default))
                .into_iter()
                .collect(),
        ),
    };
    let mute = |ui: &mut Ui, cx: &mut Cx<'_>, place: Rect| {
        if mute_button(ui, place, title, now.muted) {
            cx.edits
                .push(vec![set(name, None, "mute", json!(!now.muted))]);
        }
    };
    cx.group_with(ui, group, mute, |ui, cx| {
        if now.muted {
            ui.multiply_opacity(0.5);
        }
        ui.push_id(name, |ui| level_rows(ui, cx, name, &now));
    });
}

/// Volume, gain, normalization and, for the microphone, noise suppression of the default of
/// `name`.
fn level_rows(ui: &mut Ui, cx: &mut Cx<'_>, name: &str, now: &Levels) {
    if let Some(v) = volume(ui, text::VOLUME, now.volume) {
        cx.edits.push(vec![set(name, None, "volume", json!(v))]);
    }
    if let Some(v) = gain(ui, text::GAIN, now.gain) {
        cx.edits.push(vec![set(name, None, "gain", json!(v))]);
    }
    if let Some(on) = rows::toggle_hint(ui, text::NORMALIZE, now.normalize, text::LUFS) {
        cx.edits.push(vec![set(name, None, "normalize", json!(on))]);
    }
    // Only the microphone is denoised (`audio channel` refuses it elsewhere).
    if name == DENOISED
        && let Some(on) = rows::toggle(ui, text::DENOISE, now.denoise)
    {
        cx.edits.push(vec![set(name, None, "denoise", json!(on))]);
    }
}

/// The volume slider, percent 0..100.
pub(crate) fn volume(ui: &mut Ui, label: &str, value: f64) -> Option<f64> {
    rows::slider(ui, label, value, 0.0..=100.0, (1.0, names::PERCENT))
}

/// The gain slider, times 1..10.
pub(crate) fn gain(ui: &mut Ui, label: &str, value: f64) -> Option<f64> {
    rows::slider(ui, label, value, 1.0..=10.0, (0.1, names::TIMES))
}

/// The mute button of a track in `place`: a speaker, a red crossed one while muted. Clicked.
fn mute_button(ui: &mut Ui, place: Rect, title: &str, muted: bool) -> bool {
    let (icon, label) = match muted {
        true => (Icon::Mute, text::unmute(title)),
        false => (Icon::Speaker, text::mute(title)),
    };
    let mut button = IconButton::new(icon, &label).size(BUTTON, CHEVRON);
    if muted {
        button = button.color(tokens::REC);
    }
    let mut cell = ui.new_child(UiBuilder::new().max_rect(place));
    cell.add(button).on_hover_text(&label).clicked()
}

/// `audio channel <name> [--clip <clip>] --<field> <value>`.
pub(crate) fn set(name: &str, clip: Option<ClipId>, field: &str, value: Value) -> dc_core::Step {
    let mut args = Map::new();
    args.insert("channel".to_owned(), json!(name));
    if let Some(clip) = clip {
        args.insert("clip".to_owned(), json!(clip));
    }
    args.insert(field.to_owned(), value);
    dc_core::Step::of(OP, args)
}

/// Every level of the default of `name` as arguments of `audio channel` (noise suppression
/// only for the microphone).
fn all_args(name: &str, l: &Levels) -> Map<String, Value> {
    let mut all = args([
        ("channel", json!(name)),
        ("volume", json!(l.volume)),
        ("gain", json!(l.gain)),
        ("mute", json!(l.muted)),
        ("normalize", json!(l.normalize)),
    ]);
    if name == DENOISED {
        all.insert("denoise".to_owned(), json!(l.denoise));
    }
    all
}
