//! One writer per project. The writer holds an OS lock on `lock`; the OS drops it when the process
//! exits or crashes, so a stale lock cannot exist. `lock-owner.json` only tells others who holds it
//! (a locked file cannot be read on Windows, hence the separate file).

use std::fs::{File, OpenOptions, TryLockError};

use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::dir::ProjectDir;
use crate::error::{ProjectError, Result};

const LOCK_FILE: &str = "lock";
const OWNER_FILE: &str = "lock-owner.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockOwner {
    /// «редактор DevCast», «devcast apply» and so on.
    pub app: String,
    pub pid: u32,
}

/// Held while the project is open for writing. Dropping it releases the lock.
#[derive(Debug)]
pub struct ProjectLock {
    _file: File,
}

impl ProjectLock {
    pub fn acquire(dir: &ProjectDir, app: &str) -> Result<Self> {
        let path = dir.file(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(ProjectError::io("открыть", &path))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(ProjectError::Locked {
                    holder: holder(dir),
                });
            }
            Err(TryLockError::Error(e)) => {
                return Err(ProjectError::io("заблокировать", &path)(e));
            }
        }
        let owner = LockOwner {
            app: app.to_owned(),
            pid: std::process::id(),
        };
        atomic::write_json(&dir.file(OWNER_FILE), &owner)?;
        Ok(Self { _file: file })
    }
}

impl LockOwner {
    /// The last writer `lock-owner.json` names: a hint, the lock may be free by now.
    pub fn of(dir: &ProjectDir) -> Option<Self> {
        std::fs::read_to_string(dir.file(OWNER_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
    }
}

/// Who holds the lock, for the error message.
fn holder(dir: &ProjectDir) -> String {
    LockOwner::of(dir).map_or_else(
        || "другой процесс".to_owned(),
        |o| format!("{} (pid {})", o.app, o.pid),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_writer_is_refused_with_the_holder_named() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("Проект.devcast")).unwrap();
        let _held = ProjectLock::acquire(&dir, "редактор DevCast").unwrap();
        let error = ProjectLock::acquire(&dir, "devcast apply")
            .unwrap_err()
            .to_string();
        assert!(error.contains("редактор DevCast (pid"), "{error}");
    }

    #[test]
    fn lock_of_a_finished_writer_is_free() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("Проект.devcast")).unwrap();
        assert_eq!(LockOwner::of(&dir), None);
        drop(ProjectLock::acquire(&dir, "первый").unwrap());
        let owner = LockOwner::of(&dir).map(|o| o.app);
        assert_eq!(
            owner.as_deref(),
            Some("первый"),
            "owner info of a finished writer stays behind"
        );
        ProjectLock::acquire(&dir, "второй").unwrap();
    }
}
