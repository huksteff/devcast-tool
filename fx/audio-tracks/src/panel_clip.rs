//! The sound of a picked clip (mockup 3d, «Клип»), picked on the clips row or on its stretch of
//! «Микрофон» / «Системный звук»: «Весь клип» — sound or silence (`audio mute`), then per track
//! of the clip its own volume, gain, mute, normalization and, for the microphone, noise
//! suppression (`audio channel <track> --clip`). A recording's tracks are `mic` and `system`, an
//! imported video's `audio-1`, `audio-2` … ([`Sources::streams`]). A level the clip never set
//! follows the track's default (decision 000); the caption of the track names the levels of its
//! own. «Сбросить» — `audio reset` (every level back to the defaults) and sound on, one undo
//! step.

use dc_core::OpName;
use dc_core::{ChannelLevels, Clip, ClipId, Step, project::MediaKind};
use dc_ui_kit::egui::Ui;
use dc_ui_kit::panel::{Cx, Group, rows};
use dc_ui_kit::strings::sound as text;
use serde_json::{Map, json};

use crate::panel::{self, Levels, Sources};
use crate::{Audio, DENOISED};

const AUDIO_MUTE: OpName = OpName::of::<crate::AudioMute>();
const AUDIO_RESET: OpName = OpName::of::<crate::AudioReset>();

/// The panel of clip `id` under the header the editor draws; edits go to `cx.edits`.
pub fn clip(ui: &mut Ui, cx: &mut Cx<'_>, id: ClipId, sources: &Sources<'_>) {
    let Some(clip) = cx.project.clips.iter().find(|c| c.id == id).cloned() else {
        return;
    };
    // A broken sound state is a problem the timeline shows already: the defaults read neutral.
    let defaults = cx
        .project
        .state::<Audio>()
        .map(|s| s.settings.channels)
        .unwrap_or_default();
    cx.groups.panel(panel::ID);
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
            cx.edits.push(vec![mute(id, !on)]);
        }
        let Some(tracks) = tracks(sources, &clip) else {
            return;
        };
        if clip.media.kind == MediaKind::Imported && tracks.is_empty() {
            rows::note(ui, text::NO_VIDEO_SOUND);
            return;
        }
        for (name, title) in tracks {
            let own = clip.audio.channels.get(&name).cloned().unwrap_or_default();
            let default = defaults.get(&name).cloned().unwrap_or_default();
            ui.push_id(&name, |ui| {
                track(ui, cx, (&clip, &name, &title), &own, &default)
            });
        }
        rows::note(ui, text::FOLLOWS_NOTE);
    });
}

/// The tracks of `clip`, (name, title): the recording's microphone and system sound, an imported
/// video's `audio-N` once read (`None` until then).
fn tracks(sources: &Sources<'_>, clip: &Clip) -> Option<Vec<(String, String)>> {
    match clip.media.kind {
        MediaKind::Recording => Some(
            panel::recorded(sources.recorded)
                .map(|(name, title)| (name.to_owned(), title.to_owned()))
                .collect(),
        ),
        MediaKind::Imported => Some(
            sources
                .streams?
                .iter()
                .map(|name| (name.clone(), text::video_track(name)))
                .collect(),
        ),
    }
}

/// The caption of track `name` of the clip and its rows.
fn track(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    (clip, name, title): (&Clip, &str, &str),
    own: &ChannelLevels,
    default: &ChannelLevels,
) {
    rows::caption(ui, &caption(title, own));
    let now = Levels::of(&own.over(default));
    let id = Some(clip.id);
    let mut set = |field: &str, value| cx.edits.push(vec![panel::set(name, id, field, value)]);
    if let Some(v) = panel::volume(ui, text::VOLUME, now.volume) {
        set("volume", json!(v));
    }
    if let Some(v) = panel::gain(ui, text::GAIN, now.gain) {
        set("gain", json!(v));
    }
    if let Some(on) = rows::toggle(ui, text::SILENT, now.muted) {
        set("mute", json!(on));
    }
    if let Some(on) = rows::toggle_hint(ui, text::NORMALIZE, now.normalize, text::LUFS) {
        set("normalize", json!(on));
    }
    // Only the microphone is denoised (`audio channel` refuses it elsewhere).
    if name == DENOISED
        && let Some(on) = rows::toggle(ui, text::DENOISE, now.denoise)
    {
        set("denoise", json!(on));
    }
}

/// «Микрофон · как у дорожки» or «Микрофон · свои: громкость, усиление».
fn caption(title: &str, own: &ChannelLevels) -> String {
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
fn mute(clip: ClipId, silent: bool) -> Step {
    let mut args = Map::new();
    args.insert("clip".to_owned(), json!(clip));
    if !silent {
        args.insert("off".to_owned(), json!(true));
    }
    Step::of(AUDIO_MUTE, args)
}

/// `audio reset <clip>` when it has levels of its own, `audio mute <clip> --off` when silent.
fn reset(clip: &Clip) -> Vec<Step> {
    let mut steps = Vec::new();
    if !clip.audio.channels.is_empty() {
        let mut args = Map::new();
        args.insert("clip".to_owned(), json!(clip.id));
        steps.push(Step::of(AUDIO_RESET, args));
    }
    if clip.audio.muted {
        steps.push(mute(clip.id, false));
    }
    steps
}
