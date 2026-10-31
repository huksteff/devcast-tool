//! The sound of a picked clip (mockup 3d, «Клип»), picked on the clips row or on its stretch of
//! «Микрофон» / «Системный звук»: «Весь клип» — sound or silence (`audio mute`), then per track
//! of the recording its own volume, gain and mute (`audio channel <track> --clip`). A level the
//! clip never set follows the track's default (decision 000); the caption of the track names
//! the levels of its own. «Сбросить» — `audio reset` (every level back to the defaults) and
//! sound on, one undo step.

use dc_core::{Clip, ClipId, project::MediaKind};
use dc_fx_audio_tracks::Audio;
use dc_ops::Step;
use dc_ui_kit::strings::sound as text;
use eframe::egui::Ui;
use serde_json::{Map, json};

use super::sound::{self, Levels};
use super::{Cx, Group, rows};
use crate::editor::Action;

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, id: ClipId) {
    let Some(clip) = cx.project.clips.iter().find(|c| c.id == id).cloned() else {
        return;
    };
    // A broken sound state is a problem the timeline shows already: the defaults read neutral.
    let defaults = cx
        .project
        .state::<Audio>()
        .map(|s| s.settings.channels)
        .unwrap_or_default();
    cx.panel = sound::ID;
    let group = Group {
        id: "clip",
        title: text::CLIP_SOUND,
        open: true,
        reset: Some(reset(&clip)),
    };
    cx.group(ui, group, |ui, cx| {
        let sounding = !clip.audio.muted;
        let options = [(true, text::SOUND_ON), (false, text::SOUND_OFF)];
        if let Some(on) = rows::choice(ui, text::WHOLE_CLIP, Some(sounding), &options) {
            cx.actions.push(mute(id, !on));
        }
        if clip.media.kind == MediaKind::Imported {
            rows::note(ui, text::IMPORTED_NOTE);
            return;
        }
        let tracks: Vec<_> = sound::recorded(cx.sound).collect();
        for (name, title) in tracks {
            let own = clip.audio.channels.get(name).cloned().unwrap_or_default();
            let default = defaults.get(name).cloned().unwrap_or_default();
            ui.push_id(name, |ui| {
                track(ui, cx, (&clip, name, title), &own, &default)
            });
        }
        rows::note(ui, text::FOLLOWS_NOTE);
    });
}

/// The caption of track `name` of the clip and its rows.
fn track(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    (clip, name, title): (&Clip, &str, &str),
    own: &dc_core::ChannelLevels,
    default: &dc_core::ChannelLevels,
) {
    rows::caption(ui, &caption(title, own));
    let now = Levels::of(&own.over(default));
    let id = Some(clip.id);
    if let Some(v) = sound::volume(ui, text::VOLUME, now.volume) {
        cx.actions.push(sound::set(name, id, "volume", json!(v)));
    }
    if let Some(v) = sound::gain(ui, text::GAIN, now.gain) {
        cx.actions.push(sound::set(name, id, "gain", json!(v)));
    }
    if let Some(on) = rows::toggle(ui, text::SILENT, now.muted) {
        cx.actions.push(sound::set(name, id, "mute", json!(on)));
    }
}

/// «Микрофон · как у дорожки» or «Микрофон · свои: громкость, усиление».
fn caption(title: &str, own: &dc_core::ChannelLevels) -> String {
    let fields: Vec<&str> = [
        (own.volume.is_some(), text::OWN_VOLUME),
        (own.gain.is_some(), text::OWN_GAIN),
        (own.muted.is_some(), text::OWN_MUTE),
        (own.normalize.is_some(), text::OWN_NORMALIZE),
        (own.denoise.is_some(), text::OWN_DENOISE),
    ]
    .into_iter()
    .filter_map(|(set, field)| set.then_some(field))
    .collect();
    match fields.is_empty() {
        true => text::follows(title),
        false => text::own(title, &fields),
    }
}

/// `audio mute <clip>`, with `--off` to sound it again.
fn mute(clip: ClipId, silent: bool) -> Action {
    Action::Apply(vec![mute_step(clip, silent)])
}

fn mute_step(clip: ClipId, silent: bool) -> Step {
    let mut args = Map::new();
    args.insert("clip".to_owned(), json!(clip));
    if !silent {
        args.insert("off".to_owned(), json!(true));
    }
    Step::new("audio mute", args)
}

/// `audio reset <clip>` when it has levels of its own, `audio mute <clip> --off` when silent.
fn reset(clip: &Clip) -> Vec<Step> {
    let mut steps = Vec::new();
    if !clip.audio.channels.is_empty() {
        let mut args = Map::new();
        args.insert("clip".to_owned(), json!(clip.id));
        steps.push(Step::new("audio reset", args));
    }
    if clip.audio.muted {
        steps.push(mute_step(clip.id, false));
    }
    steps
}
