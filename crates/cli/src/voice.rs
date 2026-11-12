//! Is there a voice in the recordings of a project (`dc_audio::voice`): `devcast info` says it,
//! `devcast cut --silences` refuses to cut the pauses of a recording without one — there every
//! moment is a pause — and `devcast silences` warns that what it lists are not pauses of speech.

use dc_audio::{AudioError, tracks, voice};
use dc_core::Project;
use dc_core::project::MediaKind;
use dc_project::ProjectDir;
use serde_json::{Value, json};

use crate::speech::MIC;

/// A recording or a video of the main track without a voice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mute {
    /// Its path in the project: `media/rec-1`.
    pub media: String,
    pub why: &'static str,
}

impl Mute {
    pub(crate) fn line(&self) -> String {
        format!("{}: {}", self.media, self.why)
    }

    pub(crate) fn json(&self) -> Value {
        json!({"media": self.media, "why": self.why})
    }
}

/// The media of the main track of `project` without a voice, in the order of the clips: a
/// recording — by its microphone, a video — by its first sound track.
pub(crate) fn mute(dir: &ProjectDir, project: &Project) -> Result<Vec<Mute>, AudioError> {
    let mut found: Vec<Mute> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for clip in &project.clips {
        let media = &clip.media;
        if seen.contains(&media.path.as_str()) {
            continue;
        }
        seen.push(&media.path);
        let tracks = tracks(dir, media)?;
        let (track, none, quiet) = match media.kind {
            MediaKind::Recording => (
                tracks.iter().find(|t| t.name == MIC),
                "записано без микрофона",
                "микрофон молчал всю запись",
            ),
            MediaKind::Imported => (
                tracks.first(),
                "в видео нет звука",
                "в звуке видео нет речи",
            ),
        };
        let why = match track {
            None => none,
            Some(track) if !voice::cached(dir, track)?.is_heard() => quiet,
            Some(_) => continue,
        };
        found.push(Mute {
            media: media.path.clone(),
            why,
        });
    }
    Ok(found)
}
