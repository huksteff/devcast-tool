//! Crash reports in `%APPDATA%\DevCast\logs\crashes` (8.5): the minidumps of `crash.rs` and the
//! panics of this module. The next start tells about reports newer than the last one it told
//! about (`AppSettings::crash_seen`), once.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;

/// Unix seconds of the newest report in `dir` when it is newer than `seen`; none without
/// reports or without the folder.
pub fn newer_than(dir: &Path, seen: u64) -> Result<Option<u64>, AppError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::settings_file(dir, error)),
    };
    let mut newest = None;
    for entry in entries {
        let modified = entry
            .and_then(|e| e.metadata())
            .and_then(|m| m.modified())
            .map_err(|e| AppError::settings_file(dir, e))?;
        newest = newest.max(Some(seconds(modified)));
    }
    Ok(newest.filter(|&newest| newest > seen))
}

/// Writes the panic `text` as a report into `dir`: a panic ends the app without a minidump.
pub fn write_panic(dir: &Path, text: &str) -> Result<(), AppError> {
    std::fs::create_dir_all(dir).map_err(|e| AppError::settings_file(dir, e))?;
    let file = dir.join(format!(
        "devcast-app-{}-panic.txt",
        seconds(SystemTime::now())
    ));
    std::fs::write(&file, text).map_err(|e| AppError::settings_file(&file, e))
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_report_is_told_once() {
        let dir = tempfile::tempdir().unwrap();
        let crashes = dir.path().join("логи с пробелом").join("crashes");
        assert_eq!(newer_than(&crashes, 0).unwrap(), None, "no folder yet");
        write_panic(&crashes, "паника: тест").unwrap();
        let newest = newer_than(&crashes, 0).unwrap().unwrap();
        assert!(newest > 0);
        assert_eq!(newer_than(&crashes, newest).unwrap(), None);
        let report = std::fs::read_dir(&crashes)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(report.path()).unwrap(),
            "паника: тест"
        );
    }
}
