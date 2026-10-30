//! `audio channel`, `audio reset`, `audio mute`: levels of the recordings' sound tracks — the
//! project defaults and the clip's own (decision 000: a clip follows the defaults until it sets
//! a level itself; that level is its own from then on).

use dc_core::project::MediaKind;
use dc_core::{ChannelLevels, Clip, ClipId, Command, CommandError, Diff, Op, Project};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::Audio;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioChannel {
    /// Sound track: system and mic of a recording, audio-1, audio-2 … of an imported video.
    pub channel: String,
    /// Clip whose own level changes (the rest of its levels keep following the defaults);
    /// without it, the default of every clip.
    pub clip: Option<ClipId>,
    /// Volume, percent, 0..100.
    pub volume: Option<f64>,
    /// Gain on top of the volume, times, 1..10; the limiter keeps peaks under −1 dBTP.
    pub gain: Option<f64>,
    /// Silent.
    pub mute: Option<bool>,
    /// Bring to −16 LUFS (EBU R128) before the volume and gain.
    pub normalize: Option<bool>,
    /// Suppress noise (RNNoise) while mixing; the recording is not changed.
    pub denoise: Option<bool>,
}

impl Command for AudioChannel {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        check_name(&self.channel)?;
        let change = ChannelLevels {
            volume: self.volume,
            gain: self.gain,
            muted: self.mute,
            normalize: self.normalize,
            denoise: self.denoise,
        };
        if let Some(problem) = change.problem() {
            return Err(CommandError::new(problem));
        }
        let Some(id) = self.clip else {
            let mut state = project.state::<Audio>()?;
            let channels = &mut state.settings.channels;
            let old = channels.get(&self.channel).cloned().unwrap_or_default();
            let new = change.over(&old);
            if new == old {
                return Ok(Diff::default());
            }
            let note = format!(
                "звук {} по умолчанию: {}",
                self.channel,
                changes(&old, &new)
            );
            channels.insert(self.channel.clone(), new);
            project.set_state::<Audio>(&state)?;
            return Ok(Diff::note(note));
        };
        let clip = clip_mut(project, id)?;
        check_track(clip, &self.channel)?;
        let old = clip.audio.channels.get(&self.channel).cloned();
        let new = change.over(&old.clone().unwrap_or_default());
        if Some(&new) == old.as_ref() || new.is_empty() {
            return Ok(Diff::default());
        }
        clip.audio.channels.insert(self.channel.clone(), new);
        Ok(Diff::changed(id))
    }
}

impl Op for AudioChannel {
    const NAME: &'static str = "audio channel";
    const ABOUT: &'static str = "Уровень дорожки записи (system, mic): громкость, усиление, mute, нормализация, шумоподавление; по умолчанию или у клипа --clip";
    const EXAMPLE: &'static str = "audio channel mic --volume 80 --normalize --denoise";
    const POSITIONAL: &'static [&'static str] = &["channel"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioReset {
    /// Clip whose track levels go back to the defaults.
    pub clip: ClipId,
    /// Only this sound track (system, mic, audio-1 …); without it, every track of the clip.
    pub channel: Option<String>,
}

impl Command for AudioReset {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let clip = clip_mut(project, self.clip)?;
        let removed = match &self.channel {
            Some(name) => clip.audio.channels.remove(name).is_some(),
            None => !std::mem::take(&mut clip.audio.channels).is_empty(),
        };
        Ok(if removed {
            Diff::changed(self.clip)
        } else {
            Diff::default()
        })
    }
}

impl Op for AudioReset {
    const NAME: &'static str = "audio reset";
    const ABOUT: &'static str =
        "Уровни дорожек клипа снова по умолчанию (mute клипа целиком не меняется)";
    const EXAMPLE: &'static str = "audio reset c2 --channel mic";
    const POSITIONAL: &'static [&'static str] = &["clip"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioMute {
    /// Clip to mute.
    pub clip: ClipId,
    /// Sound the clip again.
    #[serde(default)]
    pub off: bool,
}

impl Command for AudioMute {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let clip = clip_mut(project, self.clip)?;
        if clip.audio.muted == !self.off {
            return Ok(Diff::default());
        }
        clip.audio.muted = !self.off;
        Ok(Diff::changed(self.clip))
    }
}

impl Op for AudioMute {
    const NAME: &'static str = "audio mute";
    const ABOUT: &'static str = "Заглушить клип целиком (все его дорожки); --off — вернуть звук";
    const EXAMPLE: &'static str = "audio mute c2";
    const POSITIONAL: &'static [&'static str] = &["clip"];
}

fn clip_mut(project: &mut Project, id: ClipId) -> Result<&mut Clip, CommandError> {
    let ids: Vec<ClipId> = project.clips.iter().map(|c| c.id).collect();
    project
        .clip_mut(id)
        .ok_or_else(|| CommandError::not_found(id, ids))
}

/// `system`, `mic` or `audio-<n>`, `n` from 1.
fn check_name(name: &str) -> Result<(), CommandError> {
    let imported = name
        .strip_prefix("audio-")
        .is_some_and(|n| n.parse::<u32>().is_ok_and(|n| n > 0) && !n.starts_with('0'));
    if matches!(name, "system" | "mic") || imported {
        return Ok(());
    }
    Err(CommandError::new(format!(
        "нет дорожки звука «{name}»: у записи — system и mic, у импортированного видео — audio-1, audio-2 …"
    )))
}

/// A recording has `system` and `mic`, an imported video `audio-<n>`.
fn check_track(clip: &Clip, name: &str) -> Result<(), CommandError> {
    let recording = matches!(name, "system" | "mic");
    match (clip.media.kind, recording) {
        (MediaKind::Recording, true) | (MediaKind::Imported, false) => Ok(()),
        (MediaKind::Recording, false) => Err(CommandError::new(format!(
            "{} — запись: её дорожки system и mic, а не {name}",
            clip.id
        ))),
        (MediaKind::Imported, true) => Err(CommandError::new(format!(
            "{} — импортированное видео: его дорожки audio-1, audio-2 …, а не {name}",
            clip.id
        ))),
    }
}

/// `громкость 100→80 %, шумоподавление вкл`.
fn changes(old: &ChannelLevels, new: &ChannelLevels) -> String {
    let number = |name: &str, unit: &str, a: Option<f64>, b: Option<f64>, neutral: f64| {
        (a != b).then(|| {
            let (a, b) = (a.unwrap_or(neutral), b.unwrap_or(neutral));
            format!("{name} {a}→{b}{unit}")
        })
    };
    let switch = |name: &str, a: Option<bool>, b: Option<bool>| {
        (a != b).then(|| {
            format!(
                "{name} {}",
                if b == Some(true) {
                    "вкл"
                } else {
                    "выкл"
                }
            )
        })
    };
    let parts = [
        number("громкость", " %", old.volume, new.volume, 100.0),
        number("усиление", "×", old.gain, new.gain, 1.0),
        switch("mute", old.muted, new.muted),
        switch("нормализация", old.normalize, new.normalize),
        switch("шумоподавление", old.denoise, new.denoise),
    ];
    parts.into_iter().flatten().collect::<Vec<_>>().join(", ")
}
