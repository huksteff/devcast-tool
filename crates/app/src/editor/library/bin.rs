//! The Windows Recycle Bin of the library (crate `trash`, owner decision 2026-11-02): recordings
//! are moved there, never deleted; «Отменить» brings back the last batch while the editor is open.

use std::path::PathBuf;

use dc_ui_kit::strings::library as text;

/// Moves the project folders `roots` to the Recycle Bin.
pub fn trash(roots: &[PathBuf]) -> Result<(), String> {
    trash::delete_all(roots).map_err(|e| text::trash_failed(&e.to_string()))
}

/// Brings `roots` back from the Recycle Bin: of each, the one put there last.
pub fn restore(roots: &[PathBuf]) -> Result<(), String> {
    let failed = |e: trash::Error| text::restore_failed(&e.to_string());
    let items = trash::os_limited::list().map_err(failed)?;
    let mut found = Vec::new();
    for root in roots {
        let item = items
            .iter()
            .filter(|i| i.original_path() == *root)
            .max_by_key(|i| i.time_deleted)
            .ok_or_else(|| {
                let name = root.file_name().unwrap_or_default().to_string_lossy();
                text::restore_failed(&text::not_in_bin(&name))
            })?;
        found.push(item.clone());
    }
    trash::os_limited::restore_all(found).map_err(failed)
}
