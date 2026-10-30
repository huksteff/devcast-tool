//! Sound of the project: levels of the recordings' sound tracks (project defaults and each
//! clip's own, per decision 000), mute of a whole clip, noise suppression, EBU R128
//! normalization, and extra sound files on the output timeline.
//!
//! The model and commands live here; the mixing is dc-audio's. [`sound`] gives the levels in
//! effect as plain data, and the caller turns them into the mixer's input.

mod channel;
mod clips;
mod model;
mod sound;

pub use channel::{AudioChannel, AudioMute, AudioReset};
pub use clips::{AudioAdd, AudioRm, AudioSet};
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
        OpInfo::of::<AudioRm>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Audio>(vec![AudioKind::PREFIX])
}

#[cfg(test)]
mod clips_tests;
#[cfg(test)]
mod tests;
