//! The sound settings of a project as stored in `project.effects["audio"]`: default levels of the
//! recordings' sound tracks and the extra sound on the output timeline. The clip's own levels
//! are `Clip::audio` (dc-core): a split clip keeps them in both halves.

use std::collections::BTreeMap;

use dc_core::time::Output;
use dc_core::{ChannelLevels, Effect, Id, IdCounter, IdKind, Item, Range, TimeDelta};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Folder of the project where the files of extra sound are copied.
pub const AUDIO_DIR: &str = "assets/audio";

/// Extra sound tracks: `0..TRACKS` (as v1).
pub const TRACKS: u32 = 32;

/// Shortest extra sound clip on the output.
pub const MIN_LENGTH: TimeDelta = TimeDelta::from_millis(100);

pub struct Audio;

impl Effect for Audio {
    const ID: &'static str = "audio";
    const TITLE: &'static str = "Звук";
    type State = AudioState;
    const LISTS: &'static [&'static str] = &[AudioKind::LIST];

    fn items(state: &AudioState) -> Result<Vec<Item>, serde_json::Error> {
        state
            .clips
            .iter()
            .map(|c| Item::on_output(c.id, c.range, c))
            .collect()
    }
}

/// Id kind of extra sound clips (`au1`, `au2`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AudioKind;

impl IdKind for AudioKind {
    const PREFIX: &'static str = "au";
    const NOUN: &'static str = "звука";
    const PLURAL: &'static str = "звуки";
    const LIST: &'static str = "audio";
}

pub type AudioId = Id<AudioKind>;

/// A sound file of the project as the commands need it, read once when it was copied in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoundFile {
    /// Path in the project, `assets/audio/music.mp3`.
    pub path: String,
    /// Length of its sound.
    pub duration: TimeDelta,
}

/// A clip of extra sound: part of a file placed on the output timeline. Cutting and speed of
/// the video do not move it (decision 000) and it always plays at 1×.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioClip {
    /// Id, `au<number>`.
    pub id: AudioId,
    /// Sound file it plays.
    pub file: SoundFile,
    /// Extra track, `0..TRACKS`; clips of one track do not overlap.
    pub track: u32,
    /// Where it plays on the output video (end not included).
    pub range: Range<Output>,
    /// Time of the file played at the start of `range`, from its first sample.
    pub offset: TimeDelta,
    /// Volume, percent, `0..=100`.
    pub volume: f64,
    /// Gain on top of the volume, times, `1..=10`.
    pub gain: f64,
    /// Silent.
    pub muted: bool,
    /// Brought to −16 LUFS (EBU R128, measured on the whole file) before the volume and gain.
    pub normalize: bool,
}

/// Levels every clip of the project starts from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AudioSettings {
    /// Default levels by sound track (`system`, `mic`, `audio-1`, …); a level not set is
    /// neutral: 100 %, 1×, not muted, not normalized, no noise suppression. A clip that set a
    /// level itself keeps its own.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub channels: BTreeMap<String, ChannelLevels>,
}

/// Everything the sound effect stores in the project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AudioState {
    /// Default levels of the recordings' tracks.
    pub settings: AudioSettings,
    /// Extra sound clips in the order they were added.
    pub clips: Vec<AudioClip>,
    /// Next free sound clip id; numbers are never reused.
    pub ids: IdCounter<AudioKind>,
}

impl AudioState {
    pub fn clip_ids(&self) -> impl Iterator<Item = AudioId> + '_ {
        self.clips.iter().map(|c| c.id)
    }
}
