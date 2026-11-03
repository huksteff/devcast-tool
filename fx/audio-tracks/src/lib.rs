//! Sound of the project: levels of the recordings' sound tracks (project defaults and each
//! clip's own, per decision 000), mute of a whole clip, noise suppression, EBU R128
//! normalization, and extra sound files on the output timeline.
//!
//! The model and commands live here; the mixing is dc-audio's. [`sound`] gives the levels in
//! effect as plain data, and the caller turns them into the mixer's input.
//! The inspector panels of the sound — the «Звук» tab, a clip, an extra sound — are [`panel`].

mod channel;
mod clips;
pub mod lane;
mod model;
pub mod panel;
mod panel_clip;
mod panel_extra;
mod sound;

pub use channel::{AudioChannel, AudioMute, AudioReset, DENOISED, MIC_TRACK, SYSTEM_TRACK};
pub use clips::{AudioAdd, AudioPaste, AudioRm, AudioSet};
pub use model::{
    AUDIO_DIR, Audio, AudioClip, AudioId, AudioKind, AudioSettings, AudioState, MIN_LENGTH,
    SoundFile, TRACKS,
};
pub use sound::{ClipSound, Level, PlacedSound, Sound, sound};

use dc_core::{EffectInfo, IdKind, OpInfo};

/// The sound commands of the registry, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<AudioChannel>(),
        OpInfo::of::<AudioReset>(),
        OpInfo::of::<AudioMute>(),
        OpInfo::of::<AudioSet>(),
        OpInfo::of::<AudioPaste>(),
        OpInfo::of::<AudioRm>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Audio>(vec![AudioKind::PREFIX])
}

#[cfg(test)]
mod clips_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
