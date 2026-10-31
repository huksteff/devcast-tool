//! `image set` of a picture file, one way for `devcast image set` and the editor: the file is
//! read and copied into the project's `assets/images/` (`dc_media::import_picture`), then
//! `dc_fx_image::ImageSet` puts it into the annotation. The caller applies the command as one
//! undo step; when it is refused, `discard` deletes the copy this import made.

use std::path::{Path, PathBuf};

use dc_core::OutputTime;
use dc_fx_annotations::AnnotId;
use dc_fx_image::{IMAGES_DIR, ImageSet, PictureFile};
use dc_media::MediaError;
use dc_project::ProjectDir;

use crate::sound_file::DiscardError;

/// A picture in the project and the command that shows it.
#[derive(Debug)]
pub struct PictureImport {
    pub command: ImageSet,
    /// The copy this import made: deleted when the command is refused.
    copy: Option<PathBuf>,
}

/// Copies `source` into the project in `dir` (not with `dry_run`) and makes the `image set` of
/// annotation `id`: in the key at `at` of the output, or everywhere without it.
pub fn import_picture(
    dir: &ProjectDir,
    source: &Path,
    (id, at): (AnnotId, Option<OutputTime>),
    dry_run: bool,
) -> Result<PictureImport, MediaError> {
    let imported = dc_media::import_picture(dir, IMAGES_DIR, source, dry_run)?;
    let copy = imported.copied.then(|| {
        imported
            .path
            .split('/')
            .fold(dir.root().to_path_buf(), |p, part| p.join(part))
    });
    let durations: Vec<_> = imported.picture.frames.iter().map(|f| f.duration).collect();
    let picture = PictureFile::new(imported.path, imported.picture.size, &durations);
    let command = ImageSet { id, picture, at };
    Ok(PictureImport { command, copy })
}

impl PictureImport {
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
