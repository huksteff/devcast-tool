//! The levels in effect, for the mix (dc-audio, wired by the caller): each sound track of each
//! clip — its own level where it set one, else the project default, else neutral — and the extra
//! sound clips. Plain data: this crate does not read files.

use std::collections::BTreeMap;

use dc_core::{ChannelLevels, ClipId, OutputTime, Project, StateError, TimeDelta};

use crate::DENOISED;
use crate::model::{Audio, AudioState};

/// One sound track at the level in effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    /// Volume, `0..=1` (100 % is 1).
    pub volume: f64,
    /// Gain, times, `1..=10`.
    pub gain: f64,
    pub muted: bool,
    pub normalize: bool,
    pub denoise: bool,
}

impl Level {
    /// The level of track `name`: noise is suppressed only on the microphone, so an older
    /// project with it on elsewhere plays without it ([`DENOISED`]).
    fn of(name: &str, levels: &ChannelLevels) -> Self {
        Self {
            volume: levels.volume.unwrap_or(100.0) / 100.0,
            gain: levels.gain.unwrap_or(1.0),
            muted: levels.muted.unwrap_or(false),
            normalize: levels.normalize.unwrap_or(false),
            denoise: name == DENOISED && levels.denoise.unwrap_or(false),
        }
    }
}

/// The sound of one clip. A track not in `channels` plays neutral.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipSound {
    pub clip: ClipId,
    /// The whole clip is silent.
    pub muted: bool,
    pub channels: BTreeMap<String, Level>,
}

/// An extra sound clip as the mix plays it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSound {
    /// Path of the file in the project, `assets/audio/music.mp3`.
    pub path: String,
    /// Where it plays on the output, `start..end`.
    pub start: OutputTime,
    pub end: OutputTime,
    /// Time of the file played at `start`.
    pub offset: TimeDelta,
    pub level: Level,
}

/// Everything the mix needs from the sound settings of a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sound {
    pub clips: Vec<ClipSound>,
    pub extra: Vec<PlacedSound>,
}

/// The sound of `project`.
pub fn sound(project: &Project) -> Result<Sound, StateError> {
    let state = project.state::<Audio>()?;
    let clips = project
        .clips
        .iter()
        .map(|clip| ClipSound {
            clip: clip.id,
            muted: clip.audio.muted,
            channels: clip_levels(&state, &clip.audio.channels),
        })
        .collect();
    let extra = state
        .clips
        .iter()
        .map(|c| PlacedSound {
            path: c.file.path.clone(),
            start: c.range.start(),
            end: c.range.end(),
            offset: c.offset,
            level: Level {
                volume: c.volume / 100.0,
                gain: c.gain,
                muted: c.muted,
                normalize: c.normalize,
                denoise: false,
            },
        })
        .collect();
    Ok(Sound { clips, extra })
}

/// Levels of every track that the clip or the defaults set.
fn clip_levels(
    state: &AudioState,
    own: &BTreeMap<String, ChannelLevels>,
) -> BTreeMap<String, Level> {
    let defaults = &state.settings.channels;
    let names = defaults.keys().chain(own.keys());
    names
        .map(|name| {
            let default = defaults.get(name).cloned().unwrap_or_default();
            let levels = own.get(name).map_or(default.clone(), |o| o.over(&default));
            (name.clone(), Level::of(name, &levels))
        })
        .collect()
}
