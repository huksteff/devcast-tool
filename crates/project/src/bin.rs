//! The Windows Recycle Bin (crate `trash`, owner decision 2026-11-02): project folders are moved
//! there, never deleted, and come back to where they were. One way for `devcast trash` and the
//! bin of the editor's library.

use std::path::{Path, PathBuf};

use crate::error::{ProjectError, Result};

/// Moves the folders `roots` to the Recycle Bin. The caller makes sure no writer holds them.
pub fn trash(roots: &[PathBuf]) -> Result<()> {
    trash::delete_all(roots).map_err(|e| ProjectError::Trash {
        reason: e.to_string(),
    })
}

/// Brings `roots` (the absolute paths they had) back from the Recycle Bin: of each, the one put
/// there last.
pub fn restore(roots: &[PathBuf]) -> Result<()> {
    let found = binned(roots)?;
    trash::os_limited::restore_all(found).map_err(failed)
}

/// Fails as [`restore`] would when one of `roots` is not in the Recycle Bin; moves nothing
/// (`--dry-run`).
pub fn check_restore(roots: &[PathBuf]) -> Result<()> {
    binned(roots).map(drop)
}

fn binned(roots: &[PathBuf]) -> Result<Vec<trash::TrashItem>> {
    let items = trash::os_limited::list().map_err(failed)?;
    let mut found = Vec::new();
    for root in roots {
        let item = items
            .iter()
            .filter(|i| same_path(&i.original_path(), root))
            .max_by_key(|i| i.time_deleted)
            .ok_or_else(|| ProjectError::NotInBin { path: root.clone() })?;
        found.push(item.clone());
    }
    Ok(found)
}

/// One folder however it is written: Windows paths ignore case, and the same folder may come
/// as an 8.3 short name (`C:\Users\4D88~1\…`, as `%TEMP%` of a non-Latin user name) while the
/// bin keeps its long one. The parent folder still exists, so it is compared as the file system
/// resolves it.
fn same_path(a: &Path, b: &Path) -> bool {
    let key = |p: &Path| resolved(p).to_string_lossy().to_lowercase();
    key(a) == key(b)
}

/// `path` with its parent folder resolved by the file system; as written when it can not be.
fn resolved(path: &Path) -> PathBuf {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return path.to_path_buf();
    };
    match std::fs::canonicalize(parent) {
        Ok(parent) => parent.join(name),
        Err(_) => path.to_path_buf(),
    }
}

fn failed(e: trash::Error) -> ProjectError {
    ProjectError::Restore {
        reason: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::same_path;

    #[test]
    fn a_folder_is_the_same_however_its_parent_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("Проект");
        std::fs::create_dir(&sub).unwrap();
        let long = dir.path().join("демо.devcast");
        let roundabout = sub.join("..").join("ДЕМО.devcast");
        assert!(same_path(&long, &roundabout));
        assert!(!same_path(&long, &dir.path().join("другое.devcast")));
    }
}
