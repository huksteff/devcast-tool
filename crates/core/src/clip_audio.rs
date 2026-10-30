//! Sound settings of a clip (SCOPE «Аудио», decision 000 «Звук»). They are settings of the clip
//! like its crop, so they live in [`crate::Clip`] and a split clip keeps them in both halves; the
//! commands and the project defaults they fall back to belong to `fx/audio-tracks`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Loudest volume, percent.
pub const VOLUME_MAX: f64 = 100.0;
/// Smallest and largest gain, times.
pub const GAIN_MIN: f64 = 1.0;
pub const GAIN_MAX: f64 = 10.0;

/// Levels of one sound track; a field that is `None` is not set here and comes from below (the
/// project default for a clip, neutral for a default).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ChannelLevels {
    /// Volume, percent, `0..=VOLUME_MAX`; neutral 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    /// Gain on top of the volume, times, `GAIN_MIN..=GAIN_MAX`; neutral 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain: Option<f64>,
    /// Silent; neutral `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    /// Brought to −16 LUFS (EBU R128) before the volume and gain; neutral `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalize: Option<bool>,
    /// Noise suppressed (RNNoise) while mixing, the file is not changed; neutral `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denoise: Option<bool>,
}

impl ChannelLevels {
    /// These levels where set, `under` elsewhere.
    pub fn over(&self, under: &Self) -> Self {
        Self {
            volume: self.volume.or(under.volume),
            gain: self.gain.or(under.gain),
            muted: self.muted.or(under.muted),
            normalize: self.normalize.or(under.normalize),
            denoise: self.denoise.or(under.denoise),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Why these levels cannot be: a volume or gain out of its range.
    pub fn problem(&self) -> Option<String> {
        if let Some(volume) = self.volume
            && !(0.0..=VOLUME_MAX).contains(&volume)
        {
            return Some(format!(
                "громкость {volume} % вне 0–{VOLUME_MAX}: задай громкость от 0 до 100"
            ));
        }
        if let Some(gain) = self.gain
            && !(GAIN_MIN..=GAIN_MAX).contains(&gain)
        {
            return Some(format!(
                "усиление {gain}× вне {GAIN_MIN}–{GAIN_MAX}: задай усиление от 1 до 10"
            ));
        }
        None
    }
}

/// Sound settings of a clip.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ClipAudio {
    /// The whole clip is silent; its tracks keep their levels for when it is not.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
    /// Levels the clip changed, by sound track (`system`, `mic` of a recording, `audio-1`, … of
    /// an imported video). A changed level is the clip's own from then on; the rest follow the
    /// project defaults.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub channels: BTreeMap<String, ChannelLevels>,
}

impl ClipAudio {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn problems(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter_map(|(name, levels)| levels.problem().map(|p| format!("звук {name}: {p}")))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_values_cover_the_ones_below() {
        let clip = ChannelLevels {
            volume: Some(50.0),
            muted: Some(false),
            ..ChannelLevels::default()
        };
        let default = ChannelLevels {
            volume: Some(80.0),
            gain: Some(2.0),
            muted: Some(true),
            ..ChannelLevels::default()
        };
        let both = clip.over(&default);
        assert_eq!(both.volume, Some(50.0));
        assert_eq!(both.gain, Some(2.0));
        assert_eq!(both.muted, Some(false));
        assert_eq!(both.normalize, None);
    }

    #[test]
    fn out_of_range_levels_are_problems() {
        let loud = ChannelLevels {
            volume: Some(101.0),
            ..ChannelLevels::default()
        };
        assert!(loud.problem().unwrap().contains("от 0 до 100"));
        let weak = ChannelLevels {
            gain: Some(0.5),
            ..ChannelLevels::default()
        };
        assert!(weak.problem().unwrap().contains("от 1 до 10"));
        let mut audio = ClipAudio::default();
        audio.channels.insert("mic".to_owned(), loud);
        assert!(audio.problems()[0].starts_with("звук mic: громкость"));
    }

    #[test]
    fn default_is_not_written() {
        let json = serde_json::to_value(ClipAudio::default()).unwrap();
        assert_eq!(json, serde_json::json!({}));
    }
}
