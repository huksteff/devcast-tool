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

/// Windows paths ignore case.
fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

fn failed(e: trash::Error) -> ProjectError {
    ProjectError::Restore {
        reason: e.to_string(),
    }
}
