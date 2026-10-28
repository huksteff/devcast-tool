//! Levels of the sound of each clip (SCOPE «Аудио», decision 000 «Звук»): every sound track
//! (channel) of a clip has a volume 0–100 %, a gain up to 10×, a mute and EBU R128
//! normalization; the clip as a whole has a mute. There is no common level of a clip.
//!
//! These are the levels in effect: which of them come from the project defaults and which the
//! clip changed is the model of `fx/audio-tracks`, which passes the result here.

use std::collections::BTreeMap;

use dc_core::ClipId;

use crate::error::AudioError;

/// Highest gain of a channel.
pub const GAIN_MAX: f32 = 10.0;

/// Level of one sound track of a clip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelLevel {
    volume: f32,
    gain: f32,
    muted: bool,
    normalized: bool,
    denoised: bool,
}

impl Default for ChannelLevel {
    fn default() -> Self {
        Self {
            volume: 1.0,
            gain: 1.0,
            muted: false,
            normalized: false,
            denoised: false,
        }
    }
}

impl ChannelLevel {
    /// `volume` in 0–1 (0–100 %), `gain` in 1–[`GAIN_MAX`]; not muted, not normalized.
    pub fn new(volume: f32, gain: f32) -> Result<Self, AudioError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(AudioError::Level {
                what: "громкость",
                value: volume,
                range: "0–1 (0–100 %)",
            });
        }
        if !(1.0..=GAIN_MAX).contains(&gain) {
            return Err(AudioError::Level {
                what: "усиление",
                value: gain,
                range: "1–10",
            });
        }
        Ok(Self {
            volume,
            gain,
            ..Self::default()
        })
    }

    pub fn with_mute(self, muted: bool) -> Self {
        Self { muted, ..self }
    }

    /// Brought to −16 LUFS (EBU R128) before the volume and gain.
    pub fn with_normalize(self, normalized: bool) -> Self {
        Self { normalized, ..self }
    }

    /// Noise suppressed (RNNoise) before everything else; normalization is still measured on
    /// the track as recorded.
    pub fn with_denoise(self, denoised: bool) -> Self {
        Self { denoised, ..self }
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    pub fn normalized(&self) -> bool {
        self.normalized
    }

    pub fn denoised(&self) -> bool {
        self.denoised
    }

    /// The factor of volume, gain and mute (normalization is measured by the mix).
    pub fn factor(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume * self.gain
        }
    }
}

/// Levels of one clip: a channel not listed plays at [`ChannelLevel::default`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipLevels {
    pub muted: bool,
    /// By track name: `system`, `mic`, `audio-1`, …
    pub channels: BTreeMap<String, ChannelLevel>,
}

/// Levels of the clips of a project: a clip not listed plays at [`ClipLevels::default`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Levels {
    pub clips: BTreeMap<ClipId, ClipLevels>,
}

impl Levels {
    /// Level of track `track` of clip `clip`, with the clip's mute applied.
    pub fn channel(&self, clip: ClipId, track: &str) -> ChannelLevel {
        let Some(levels) = self.clips.get(&clip) else {
            return ChannelLevel::default();
        };
        let channel = levels.channels.get(track).copied().unwrap_or_default();
        channel.with_mute(channel.muted || levels.muted)
    }
}
