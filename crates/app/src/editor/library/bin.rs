//! The Recycle Bin of the library (`dc_project::bin`, shared with `devcast trash`): recordings
//! are moved there, never deleted; «Отменить» brings back the last batch while the editor is open.

use std::path::PathBuf;

use dc_project::ProjectError;
use dc_ui_kit::strings::library as text;

/// Moves the project folders `roots` to the Recycle Bin.
pub fn trash(roots: &[PathBuf]) -> Result<(), String> {
    dc_project::bin::trash(roots).map_err(shown)
}

/// Brings `roots` back from the Recycle Bin: of each, the one put there last.
pub fn restore(roots: &[PathBuf]) -> Result<(), String> {
    dc_project::bin::restore(roots).map_err(shown)
}

/// The words of the window for what went wrong.
fn shown(error: ProjectError) -> String {
    match error {
        ProjectError::Trash { reason } => text::trash_failed(&reason),
        ProjectError::Restore { reason } => text::restore_failed(&reason),
        ProjectError::NotInBin { path } => {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            text::restore_failed(&text::not_in_bin(&name))
        }
        other => other.to_string(),
    }
}
