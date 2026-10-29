//! Derived data in the project's `cache/`, safe to delete: a file of the project is cached at
//! its own relative path plus a suffix, `media/recording-1/mic.flac` →
//! `cache/media/recording-1/mic.flac<suffix>`. A cache entry stays valid while its file keeps
//! the same [`FileStamp`].

use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::dir::ProjectDir;
use crate::error::{ProjectError, Result};

/// Size and modification time of a file: a cache built from it is stale once either changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    pub size: u64,
    /// Nanoseconds from the Unix epoch, negative before it.
    pub modified_ns: i128,
}

impl FileStamp {
    pub fn of(file: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(file).map_err(ProjectError::io("прочитать", file))?;
        let modified = metadata
            .modified()
            .map_err(ProjectError::io("узнать время изменения", file))?;
        let modified_ns = match modified.duration_since(UNIX_EPOCH) {
            Ok(after) => after.as_nanos() as i128,
            Err(before) => -(before.duration().as_nanos() as i128),
        };
        Ok(Self {
            size: metadata.len(),
            modified_ns,
        })
    }

    /// Little-endian bytes for binary cache headers.
    pub fn to_le_bytes(self) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[..8].copy_from_slice(&self.size.to_le_bytes());
        bytes[8..].copy_from_slice(&self.modified_ns.to_le_bytes());
        bytes
    }
}

impl ProjectDir {
    /// Where the cache of `file` with `suffix` lies. `file` is relative to the project (with `/`
    /// or `\`) or [`ProjectDir::root`] joined with such a path.
    pub fn cache_file(&self, file: &Path, suffix: &str) -> PathBuf {
        let relative = file.strip_prefix(self.root()).unwrap_or(file);
        let mut path = self.cache_dir();
        path.extend(relative.components().filter_map(|c| match c {
            Component::Normal(part) => Some(part),
            _ => None,
        }));
        let mut name = path.into_os_string();
        name.push(suffix);
        PathBuf::from(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_mirrors_the_path_in_the_project() {
        let tmp = tempfile::tempdir().unwrap();
        let project = ProjectDir::create(tmp.path().join("Кэш проекта.dcproj")).unwrap();
        let expected = project
            .cache_dir()
            .join("media")
            .join("запись 1")
            .join("mic.flac.peaks");
        let relative = project.cache_file(Path::new("media/запись 1/mic.flac"), ".peaks");
        let joined = project
            .root()
            .join("media")
            .join("запись 1")
            .join("mic.flac");
        assert_eq!(relative, expected);
        assert_eq!(project.cache_file(&joined, ".peaks"), expected);
    }

    #[test]
    fn stamp_changes_with_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("звук.flac");
        std::fs::write(&file, b"sound").unwrap();
        let before = FileStamp::of(&file).unwrap();
        assert_eq!(before.size, 5);
        std::fs::write(&file, b"other sound").unwrap();
        assert_ne!(FileStamp::of(&file).unwrap(), before);
    }
}
