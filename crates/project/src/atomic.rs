//! Atomic file replacement: write a temporary file next to the target, flush it to disk, then
//! rename over the target. A crash leaves either the old file or the new one, never a mix. The
//! temporary file is named after the process, so the app and the CLI do not open each other's.
//! Writes of one process go one at a time: threads of the app fill one probe cache, and on
//! Windows two renames over the same file at once fail with «access denied».
//! On Windows an antivirus or the search index opens every new file for a moment, and creating
//! or renaming a file it holds fails. That holder is not ours and Windows has no call to wait
//! for it, so the write tries again for up to [`BUSY_WAIT`] and then tells the error.

use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::error::{ProjectError, Result};

static WRITING: Mutex<()> = Mutex::new(());

/// How long a write waits for a file that another program holds open.
const BUSY_WAIT: Duration = Duration::from_secs(2);
const BUSY_STEP: Duration = Duration::from_millis(10);

pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let _one_at_a_time = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    {
        let mut file =
            patient(|| std::fs::File::create(&tmp)).map_err(ProjectError::io("создать", &tmp))?;
        file.write_all(bytes)
            .map_err(ProjectError::io("записать", &tmp))?;
        file.sync_all()
            .map_err(ProjectError::io("сбросить на диск", &tmp))?;
    }
    // On Windows `rename` replaces an existing target (MoveFileEx with REPLACE_EXISTING).
    patient(|| std::fs::rename(&tmp, path)).map_err(ProjectError::io("переименовать", &tmp))?;
    sync_dir(path)
}

/// Runs `act` again while another program holds its file, for up to [`BUSY_WAIT`].
fn patient<T>(mut act: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let started = Instant::now();
    loop {
        match act() {
            Err(e) if held(&e) && started.elapsed() < BUSY_WAIT => std::thread::sleep(BUSY_STEP),
            done => return done,
        }
    }
}

/// Another program holds the file open: `ERROR_ACCESS_DENIED` (5) or `ERROR_SHARING_VIOLATION`
/// (32).
#[cfg(windows)]
fn held(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32))
}

#[cfg(not(windows))]
fn held(_: &std::io::Error) -> bool {
    false
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

    /// Opens `path` the way a scanner does: nobody else may write, rename or delete it.
    #[cfg(windows)]
    fn hold(path: &Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn waits_for_a_program_that_holds_the_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let tmp = dir
            .path()
            .join(format!("manifest.json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, b"left").unwrap();
        let held = hold(&tmp);
        let scanner = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            drop(held);
        });
        write(&path, b"new").unwrap();
        scanner.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[cfg(windows)]
    #[test]
    fn waits_for_a_program_that_holds_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        write(&path, b"old").unwrap();
        let held = hold(&path);
        let scanner = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            drop(held);
        });
        write(&path, b"new").unwrap();
        scanner.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[cfg(windows)]
    #[test]
    fn a_file_held_for_good_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        write(&path, b"old").unwrap();
        let _held = hold(&path);
        assert!(write(&path, b"new").is_err());
    }
}
