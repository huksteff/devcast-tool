//! Renaming the project: its folder `name.devcast` gets another name in the same place (owner
//! decision on 7.1: the name of a project is its folder's, there is no undo).

use std::path::PathBuf;

use crate::dir::ProjectDir;
use crate::error::{ProjectError, Result};
use crate::lock::ProjectLock;

const EXTENSION: &str = ".devcast";
/// Characters Windows does not allow in a file name.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
/// Names Windows keeps for devices, without extension and case.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

impl ProjectDir {
    /// Where the folder goes when renamed to `name` (`Урок 2` or `Урок 2.devcast`): next to it.
    /// Refuses a name Windows does not allow and a folder that is there already; the same name
    /// in another case is the same folder.
    pub fn renamed_root(&self, name: &str) -> Result<PathBuf> {
        let file = folder_name(name)?;
        let open = |e| ProjectError::io("открыть", self.root())(e);
        let full = std::path::absolute(self.root()).map_err(open)?;
        let Some(parent) = full.parent() else {
            return Err(ProjectError::NotAProject { path: full });
        };
        let target = parent.join(file);
        let real = std::fs::canonicalize(self.root()).map_err(open)?;
        let same = std::fs::canonicalize(&target).is_ok_and(|t| t == real);
        if target.exists() && !same {
            return Err(ProjectError::AlreadyExists { path: target });
        }
        Ok(target)
    }

    /// Renames the folder to `name` (see [`Self::renamed_root`]). `lock` proves no other writer
    /// has the project open; it is released before the rename, because Windows refuses to rename
    /// a folder with a file open in it, so the caller closes its own files first. The renamed
    /// project is returned unlocked.
    pub fn rename(&self, lock: ProjectLock, name: &str) -> Result<Self> {
        let target = self.renamed_root(name)?;
        drop(lock);
        std::fs::rename(self.root(), &target)
            .map_err(ProjectError::io("переименовать", self.root()))?;
        Self::open(target)
    }
}

/// `name` as a folder name with `.devcast`; given with it, it is not added twice.
fn folder_name(name: &str) -> Result<String> {
    let bad = |reason: &'static str| ProjectError::BadName {
        name: name.to_owned(),
        reason,
    };
    let trimmed = name.trim();
    let stem = match trimmed.len().checked_sub(EXTENSION.len()) {
        Some(at)
            if trimmed.is_char_boundary(at) && trimmed[at..].eq_ignore_ascii_case(EXTENSION) =>
        {
            trimmed[..at].trim_end()
        }
        _ => trimmed,
    };
    if stem.is_empty() {
        return Err(bad("оно пустое"));
    }
    if stem
        .chars()
        .any(|c| FORBIDDEN.contains(&c) || c.is_control())
    {
        return Err(bad("в нём есть знаки < > : \" / \\ | ? *"));
    }
    if stem.ends_with('.') {
        return Err(bad("оно кончается точкой"));
    }
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(bad("так Windows называет устройство"));
    }
    Ok(format!("{stem}{EXTENSION}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(parent: &std::path::Path, name: &str) -> ProjectDir {
        ProjectDir::create(parent.join(name)).unwrap()
    }

    #[test]
    fn renames_the_folder_with_everything_in_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = project(tmp.path(), "Урок 1.devcast");
        std::fs::write(dir.media_dir().join("запись.mp4"), b"video").unwrap();
        let lock = ProjectLock::acquire(&dir, "devcast").unwrap();
        let renamed = dir.rename(lock, "Урок 2 с пробелами").unwrap();
        assert_eq!(renamed.name(), "Урок 2 с пробелами.devcast");
        assert!(!dir.root().exists());
        let video = renamed.media_dir().join("запись.mp4");
        assert_eq!(std::fs::read(video).unwrap(), b"video");
        ProjectLock::acquire(&renamed, "другой").unwrap();
    }

    #[test]
    fn the_extension_is_added_once_and_case_alone_may_change() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = project(tmp.path(), "урок.devcast");
        assert_eq!(
            dir.renamed_root("  Урок.DEVCAST ").unwrap(),
            tmp.path().join("Урок.devcast")
        );
        let lock = ProjectLock::acquire(&dir, "devcast").unwrap();
        assert_eq!(dir.rename(lock, "Урок").unwrap().name(), "Урок.devcast");
    }

    #[test]
    fn refuses_a_taken_name_and_names_windows_does_not_allow() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = project(tmp.path(), "Урок 1.devcast");
        project(tmp.path(), "Урок 2.devcast");
        let lock = ProjectLock::acquire(&dir, "devcast").unwrap();
        let error = dir.rename(lock, "Урок 2").unwrap_err();
        assert!(
            matches!(error, ProjectError::AlreadyExists { .. }),
            "{error}"
        );
        assert!(dir.root().is_dir(), "nothing moved");
        for name in ["", " .devcast", "a/b", "что?", "точка.", "con", "Lpt1"] {
            let error = dir.renamed_root(name).unwrap_err();
            assert!(
                matches!(error, ProjectError::BadName { .. }),
                "{name}: {error}"
            );
        }
    }
}
