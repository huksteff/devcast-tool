//! Rust source files of the workspace that the text checks look at.

use std::path::{Path, PathBuf};

/// Folders (relative to the workspace root) that hold our Rust code.
const SOURCE_ROOTS: [&str; 3] = ["crates", "fx", "xtask"];

/// All `.rs` files under the source roots, sorted, skipping `target` folders.
pub fn rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for dir in SOURCE_ROOTS {
        collect(&root.join(dir), &mut files)?;
    }
    files.sort();
    Ok(files)
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|error| format!("не удалось прочитать папку {}: {error}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("ошибка чтения папки {}: {error}", dir.display()))?
            .path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                collect(&path, files)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// Path shown in messages: relative to the workspace root, with `/` separators.
pub fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|error| format!("не удалось прочитать {}: {error}", path.display()))
}
