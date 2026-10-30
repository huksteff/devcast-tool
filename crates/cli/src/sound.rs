//! The sound settings of a project (`fx/audio-tracks`) as the input of the mix (`dc_audio`):
//! the effect gives the levels in effect as plain data, this only changes their form.

use std::path::Path;

use dc_audio::{AudioError, ChannelLevel, ClipLevels, ExtraClip, Levels};
use dc_core::Project;
use dc_fx_audio_tracks::{Level, sound};

/// Levels of the clips and the extra sound of `project`, whose folder is `root`.
pub(crate) fn levels(root: &Path, project: &Project) -> Result<Levels, SoundError> {
    let sound = sound(project)?;
    let mut levels = Levels::default();
    for clip in sound.clips {
        let mut channels = std::collections::BTreeMap::new();
        for (name, level) in clip.channels {
            channels.insert(name, channel(level)?);
        }
        let muted = clip.muted;
        levels
            .clips
            .insert(clip.clip, ClipLevels { muted, channels });
    }
    for placed in sound.extra {
        levels.extra.push(ExtraClip {
            path: placed
                .path
                .split('/')
                .fold(root.to_path_buf(), |p, part| p.join(part)),
            start: placed.start,
            end: placed.end,
            offset: placed.offset,
            level: channel(placed.level)?,
        });
    }
    Ok(levels)
}

fn channel(level: Level) -> Result<ChannelLevel, AudioError> {
    Ok(ChannelLevel::new(level.volume as f32, level.gain as f32)?
        .with_mute(level.muted)
        .with_normalize(level.normalize)
        .with_denoise(level.denoise))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SoundError {
    #[error(transparent)]
    State(#[from] dc_core::StateError),
    #[error(transparent)]
    Audio(#[from] AudioError),
}
