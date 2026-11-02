//! Atomic file replacement: write a temporary file next to the target, flush it to disk, then
//! rename over the target. A crash leaves either the old file or the new one, never a mix. The
//! temporary file is named after the process and a number of this write, so two writers of the
//! same file (the app and the CLI, or the preview and sound threads filling one probe cache) do
//! not open each other's.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{ProjectError, Result};

/// Number of the next write of this process.
static WRITES: AtomicU64 = AtomicU64::new(0);

pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}-{write}.tmp", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    {
        let mut file = std::fs::File::create(&tmp).map_err(ProjectError::io("создать", &tmp))?;
        file.write_all(bytes)
            .map_err(ProjectError::io("записать", &tmp))?;
        file.sync_all()
            .map_err(ProjectError::io("сбросить на диск", &tmp))?;
    }
    // On Windows `rename` replaces an existing target (MoveFileEx with REPLACE_EXISTING).
    std::fs::rename(&tmp, path).map_err(ProjectError::io("переименовать", &tmp))?;
    sync_dir(path)
}

/// Makes the rename itself durable. NTFS journals metadata, so this is only needed on Unix.
#[cfg(unix)]
fn sync_dir(path: &Path) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(ProjectError::io("сбросить на диск", dir))
}

#[cfg(not(unix))]
fn sync_dir(_: &Path) -> Result<()> {
    Ok(())
}

/// Serializes `value` as pretty JSON (readable by people and agents) and writes it atomically.
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| ProjectError::Corrupt {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    text.push('\n');
    write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_file_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Проект с пробелами.json");
        write(&path, b"old").unwrap();
        write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    #[test]
    fn threads_writing_one_file_do_not_clash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("кэш probe.json");
        std::thread::scope(|scope| {
            for thread in 0..8u8 {
                let path = &path;
                scope.spawn(move || {
                    for _ in 0..50 {
                        write(path, &[thread; 64]).unwrap();
                    }
                });
            }
        });
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() == 64 && bytes.iter().all(|b| *b == bytes[0]));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
