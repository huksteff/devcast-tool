//! Importing pictures into a project (image annotations): the file is read first (a picture that
//! does not decode is not copied), then copied into a folder of the project under its own name
//! (`-2`, `-3` … when taken by another file). The same file imported again is not copied twice.

use std::path::Path;

use dc_project::ProjectDir;

use crate::error::MediaError;
use crate::picture::{Picture, decode_picture};

/// Picture files that can be imported (decision 000: PNG, JPEG, WebP, GIF; SVG is not read yet).
pub const PICTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

/// A picture put into the project.
#[derive(Debug)]
pub struct ImportedPicture {
    /// Path in the project with `/`, `assets/images/logo.png`.
    pub path: String,
    pub picture: Picture,
    /// Whether this import made the file; `false` when the same file was already there (or for
    /// a dry run). If the change that needs it fails, only a file it made is deleted.
    pub copied: bool,
}

/// Reads `source` and copies it into `folder` of the project (a path with `/` from the project
/// root). `dry_run` only reads it and names the file it would be.
pub fn import_picture(
    project: &ProjectDir,
    folder: &str,
    source: &Path,
    dry_run: bool,
) -> Result<ImportedPicture, MediaError> {
    let extension = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if extension == "svg" {
        return Err(MediaError::Svg {
            path: source.to_path_buf(),
        });
    }
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned());
    let (Some(stem), true) = (stem, PICTURE_EXTENSIONS.contains(&extension.as_str())) else {
        return Err(MediaError::NotAPicture {
            path: source.to_path_buf(),
        });
    };
    if !source.is_file() {
        return Err(MediaError::NotFound {
            path: source.to_path_buf(),
        });
    }
    let picture = decode_picture(source)?;
    let file = dc_project::copy_in(project, folder, source, &stem, &extension, dry_run)?;
    Ok(ImportedPicture {
        path: file.path,
        picture,
        copied: file.copied,
    })
}

#[cfg(test)]
#[path = "picture_import_tests.rs"]
mod tests;
