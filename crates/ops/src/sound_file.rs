//! `audio add` of a sound file, one way for `devcast audio add` and the editor: the file is read
//! whole and copied into the project's `assets/audio/` (`dc_audio::import_sound`), then
//! `dc_fx_audio_tracks::AudioAdd` places it. The caller applies the command as one undo step;
//! when it is refused, `discard` deletes the copy this import made.

use std::path::{Path, PathBuf};

use dc_audio::AudioError;
use dc_core::{OutputTime, TimeDelta};
use dc_fx_audio_tracks::{AUDIO_DIR, AudioAdd, SoundFile};
use dc_project::ProjectDir;

/// A sound file in the project and the command that places it.
#[derive(Debug)]
pub struct SoundImport {
    pub command: AudioAdd,
    /// The copy this import made: deleted when the command is refused.
    copy: Option<PathBuf>,
}

/// Where the imported sound goes: start on the output, extra track (the first free one without
/// it), length (the file to its end, cut at the end of the video, without it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundPlace {
    pub at: OutputTime,
    pub track: Option<u32>,
    pub duration: Option<TimeDelta>,
}

/// The copy of a refused sound or picture stays in the project.
#[derive(Debug, thiserror::Error)]
#[error("копию файла «{}» удалить не удалось ({source}): удали её вручную", .file.display())]
pub struct DiscardError {
    pub file: PathBuf,
    pub source: std::io::Error,
}

/// Copies `source` into the project in `dir` (not with `dry_run`) and makes its `audio add`.
pub fn import_sound(
    dir: &ProjectDir,
    source: &Path,
    place: SoundPlace,
    dry_run: bool,
) -> Result<SoundImport, AudioError> {
    let imported = dc_audio::import_sound(dir, AUDIO_DIR, source, dry_run)?;
    let copy = imported.copied.then(|| {
        imported
            .path
            .split('/')
            .fold(dir.root().to_path_buf(), |p, part| p.join(part))
    });
    let command = AudioAdd {
        file: SoundFile {
            path: imported.path,
            duration: imported.duration,
        },
        at: place.at,
        track: place.track,
        duration: place.duration,
    };
    Ok(SoundImport { command, copy })
}

impl SoundImport {
    /// Deletes the copy this import made: the command was refused.
    pub fn discard(&self) -> Result<(), DiscardError> {
        let Some(file) = &self.copy else {
            return Ok(());
        };
        std::fs::remove_file(file).map_err(|source| DiscardError {
            file: file.clone(),
            source,
        })
    }
}
