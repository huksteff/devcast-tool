//! Copying a file into a folder of the project (pictures of annotations, extra sound): under its
//! own name, `-2`, `-3` … when the name is taken by another file; the same file copied again is
//! not copied twice.

use std::path::Path;

use crate::ProjectDir;
use crate::error::{ProjectError, Result};

/// A file put into the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedFile {
    /// Path in the project with `/`, `assets/audio/music.mp3`.
    pub path: String,
    /// Whether this call made the file; `false` when the same file was already there (or for a
    /// dry run). If the change that needs it fails, only a file it made is deleted.
    pub copied: bool,
}

/// Copies `source` into `folder` of the project (a path with `/` from the project root) as
/// `<stem>.<extension>`; `extension` is the file's own, lower case. `dry_run` only names the
/// file it would be.
pub fn copy_in(
    project: &ProjectDir,
    folder: &str,
    source: &Path,
    stem: &str,
    extension: &str,
    dry_run: bool,
) -> Result<CopiedFile> {
    let dir = folder
        .split('/')
        .fold(project.root().to_path_buf(), |dir, part| dir.join(part));
    let mut n = 1;
    loop {
        let name = match n {
            1 => format!("{stem}.{extension}"),
            _ => format!("{stem}-{n}.{extension}"),
        };
        let target = dir.join(&name);
        let path = format!("{folder}/{name}");
        if !target.exists() {
            if !dry_run {
                std::fs::create_dir_all(&dir).map_err(ProjectError::io("создать папку", &dir))?;
                std::fs::copy(source, &target).map_err(ProjectError::io("скопировать", source))?;
            }
            let copied = !dry_run;
            return Ok(CopiedFile { path, copied });
        }
        if same_bytes(source, &target)? {
            return Ok(CopiedFile {
                path,
                copied: false,
            });
        }
        n += 1;
    }
}

fn same_bytes(a: &Path, b: &Path) -> Result<bool> {
    let size = |p: &Path| {
        std::fs::metadata(p)
            .map(|m| m.len())
            .map_err(ProjectError::io("прочитать", p))
    };
    if size(a)? != size(b)? {
        return Ok(false);
    }
    let read = |p: &Path| std::fs::read(p).map_err(ProjectError::io("прочитать", p));
    Ok(read(a)? == read(b)?)
}
