//! Edits of `devcast config` and `devcast hotkeys`, one way for both writers: the running app
//! applies them to its `Store` (the request `dc_ops::live::Request::Settings`), without it
//! `devcast` applies them to the file. [`apply`] is pure and all or nothing; its answer names
//! what changed.

use std::path::PathBuf;

use dc_capture::{Fps, Source};
use serde::{Deserialize, Serialize};

use crate::keymap::{Binding, taken_by};
use crate::model::{AppSettings, Countdown, Language, Listen, SpeechModel};
use crate::shortcut::Shortcut;

/// A setting of `devcast config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    RecordingsDir,
    Source,
    Fps,
    Mic,
    Audio,
    Countdown,
    HideHud,
    AutoUpdates,
    Beta,
    Model,
    Language,
    Tracks,
}

impl Key {
    pub const ALL: [Self; 12] = [
        Self::RecordingsDir,
        Self::Source,
        Self::Fps,
        Self::Mic,
        Self::Audio,
        Self::Countdown,
        Self::HideHud,
        Self::AutoUpdates,
        Self::Beta,
        Self::Model,
        Self::Language,
        Self::Tracks,
    ];

    /// The name in `devcast config`.
    pub fn name(self) -> &'static str {
        match self {
            Self::RecordingsDir => "recordings_dir",
            Self::Source => "recording.source",
            Self::Fps => "recording.fps",
            Self::Mic => "recording.mic",
            Self::Audio => "recording.audio",
            Self::Countdown => "recording.countdown",
            Self::HideHud => "hud.hide",
            Self::AutoUpdates => "updates.auto",
            Self::Beta => "updates.beta",
            Self::Model => "captions.model",
            Self::Language => "captions.language",
            Self::Tracks => "captions.tracks",
        }
    }

    pub fn find(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    /// Its value in `settings`.
    pub fn get(self, s: &AppSettings) -> Setting {
        match self {
            Self::RecordingsDir => Setting::RecordingsDir(s.recordings_dir.clone()),
            Self::Source => Setting::Source(s.recording.source),
            Self::Fps => Setting::Fps(s.recording.fps),
            Self::Mic => Setting::Mic(s.recording.microphone.clone()),
            Self::Audio => Setting::Audio(s.recording.system_audio),
            Self::Countdown => Setting::Countdown(s.countdown),
            Self::HideHud => Setting::HideHud(s.hide_hud),
            Self::AutoUpdates => Setting::AutoUpdates(s.updates.auto_check),
            Self::Beta => Setting::Beta(s.updates.beta),
            Self::Model => Setting::Model(s.speech_model),
            Self::Language => Setting::Language(s.speech_language),
            Self::Tracks => Setting::Tracks(s.speech_tracks),
        }
    }
}

/// A value of a [`Key`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Setting {
    /// Where recordings and their projects are created.
    RecordingsDir(PathBuf),
    Source(Source),
    Fps(Fps),
    /// Microphone name as the system shows it; `None` — no microphone track.
    Mic(Option<String>),
    /// System sound as its own track.
    Audio(bool),
    Countdown(Countdown),
    /// Exclude the HUD from capture.
    HideHud(bool),
    AutoUpdates(bool),
    Beta(bool),
    Model(SpeechModel),
    Language(Language),
    Tracks(Listen),
}

impl Setting {
    pub fn key(&self) -> Key {
        match self {
            Self::RecordingsDir(_) => Key::RecordingsDir,
            Self::Source(_) => Key::Source,
            Self::Fps(_) => Key::Fps,
            Self::Mic(_) => Key::Mic,
            Self::Audio(_) => Key::Audio,
            Self::Countdown(_) => Key::Countdown,
            Self::HideHud(_) => Key::HideHud,
            Self::AutoUpdates(_) => Key::AutoUpdates,
            Self::Beta(_) => Key::Beta,
            Self::Model(_) => Key::Model,
            Self::Language(_) => Key::Language,
            Self::Tracks(_) => Key::Tracks,
        }
    }

    fn put(self, s: &mut AppSettings) {
        match self {
            Self::RecordingsDir(dir) => s.recordings_dir = dir,
            Self::Source(source) => s.recording.source = source,
            Self::Fps(fps) => s.recording.fps = fps,
            Self::Mic(mic) => s.recording.microphone = mic,
            Self::Audio(on) => s.recording.system_audio = on,
            Self::Countdown(countdown) => s.countdown = countdown,
            Self::HideHud(on) => s.hide_hud = on,
            Self::AutoUpdates(on) => s.updates.auto_check = on,
            Self::Beta(on) => s.updates.beta = on,
            Self::Model(model) => s.speech_model = model,
            Self::Language(language) => s.speech_language = language,
            Self::Tracks(tracks) => s.speech_tracks = tracks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edit {
    /// `config set`.
    Set(Vec<Setting>),
    /// `config reset`: these keys to their defaults; none — every key of `config`.
    Reset(Vec<Key>),
    /// `hotkeys set`: `keys` to `binding` (`None` — cleared). A shortcut another binding has is
    /// refused, or with `swap` that binding gets the old shortcut of this one («Заменить»).
    Bind {
        binding: Binding,
        keys: Option<Shortcut>,
        swap: bool,
    },
    /// `hotkeys reset`: these shortcuts to their defaults; none — all of them.
    ResetKeys(Vec<Binding>),
}

/// The settings after an edit and what it changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub settings: AppSettings,
    pub keys: Vec<Key>,
    pub bindings: Vec<Binding>,
}

/// An edit that is refused; nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum EditError {
    #[error(
        "{keys} уже у {owner}: выбери другое сочетание или поменяй их местами (--swap, как «Заменить» в настройках)"
    )]
    Taken { keys: String, owner: String },
    #[error(
        "глобальному сочетанию {binding} нужен Ctrl или Alt (иначе клавиша пропадёт во всех программах): например Ctrl+Shift+R"
    )]
    NeedsModifier { binding: String },
}

/// `edit` of `settings`; refused — nothing changes.
pub fn apply(settings: &AppSettings, edit: Edit) -> Result<Applied, EditError> {
    let mut next = settings.clone();
    match edit {
        Edit::Set(values) => values.into_iter().for_each(|v| v.put(&mut next)),
        Edit::Reset(keys) => {
            let defaults = AppSettings::default();
            let keys = if keys.is_empty() {
                Key::ALL.to_vec()
            } else {
                keys
            };
            for key in keys {
                key.get(&defaults).put(&mut next);
            }
        }
        Edit::Bind {
            binding,
            keys,
            swap,
        } => bind(&mut next, binding, keys, swap)?,
        Edit::ResetKeys(bindings) => {
            let all: Vec<Binding> = match bindings.is_empty() {
                true => Binding::all().collect(),
                false => bindings,
            };
            all.into_iter().for_each(|b| b.reset(&mut next));
        }
    }
    Ok(Applied {
        keys: Key::ALL
            .into_iter()
            .filter(|k| k.get(settings) != k.get(&next))
            .collect(),
        bindings: Binding::all()
            .filter(|b| b.keys(settings) != b.keys(&next))
            .collect(),
        settings: next,
    })
}

/// The rules of the settings window: a global shortcut needs Ctrl or Alt, a taken one is
/// refused or swapped.
fn bind(
    s: &mut AppSettings,
    binding: Binding,
    keys: Option<Shortcut>,
    swap: bool,
) -> Result<(), EditError> {
    let Some(shortcut) = keys else {
        binding.set(s, None);
        return Ok(());
    };
    if !binding.takes(shortcut) {
        return Err(EditError::NeedsModifier {
            binding: binding.id().to_owned(),
        });
    }
    if let Some(owner) = taken_by(s, shortcut, binding) {
        let old = binding.keys(s);
        let fits = old
            .as_deref()
            .and_then(Shortcut::parse)
            .is_none_or(|old| owner.takes(old));
        if !swap {
            return Err(EditError::Taken {
                keys: shortcut.text(),
                owner: owner.id().to_owned(),
            });
        }
        if !fits {
            return Err(EditError::NeedsModifier {
                binding: owner.id().to_owned(),
            });
        }
        owner.set(s, old);
    }
    binding.set(s, Some(shortcut.text()));
    Ok(())
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
