//! Levels of the sound of each clip (SCOPE «Аудио», decision 000 «Звук»): every sound track
//! (channel) of a clip has a volume 0–100 %, a gain up to 10× and a mute; the clip as a whole
//! has a mute. There is no common level of a clip.
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
}

impl Default for ChannelLevel {
    fn default() -> Self {
        Self {
            volume: 1.0,
            gain: 1.0,
            muted: false,
        }
    }
}

impl ChannelLevel {
    /// `volume` in 0–1 (0–100 %), `gain` in 1–[`GAIN_MAX`].
    pub fn new(volume: f32, gain: f32, muted: bool) -> Result<Self, AudioError> {
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
            muted,
        })
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

    /// The factor applied to the track's samples.
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
    /// The factor of track `track` of clip `clip`.
    pub fn factor(&self, clip: ClipId, track: &str) -> f32 {
        let Some(levels) = self.clips.get(&clip) else {
            return 1.0;
        };
        if levels.muted {
            return 0.0;
        }
        levels.channels.get(track).map_or(1.0, ChannelLevel::factor)
    }
}
