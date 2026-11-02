//! The recordings of the library: the projects of the recordings folder
//! (`dc_project::library`, the list of `devcast ls projects`) without the editor's own. A
//! project that cannot be read is left out with a warning in the log.

use std::path::Path;

pub use dc_project::library::Entry;

/// The recordings in `folder` except the project `open` (the editor's own).
pub fn scan(folder: &Path, open: &Path) -> Vec<Entry> {
    let scan = dc_project::library::scan(folder);
    for (root, error) in scan.skipped {
        tracing::warn!("библиотека пропускает {}: {error}", root.display());
    }
    scan.entries
        .into_iter()
        .filter(|e| e.root != open)
        .collect()
}
