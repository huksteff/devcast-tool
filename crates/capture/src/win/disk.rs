//! Room on the disk a recording goes to: a recording does not start on a nearly full disk, and
//! a running one warns before the disk is full (a full disk cuts the recording off).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::core::PCWSTR;

use crate::error::CaptureError;

/// The least free room to start a recording, bytes: about half an hour of 1080p60.
pub const ROOM_TO_START: u64 = 500 * MB;
/// Below this a running recording warns, bytes: about ten minutes are left.
pub const ROOM_LOW: u64 = 200 * MB;
const MB: u64 = 1024 * 1024;

/// Bytes free for this user on the disk of `path`, an existing folder or the nearest one above
/// a folder that is not made yet.
pub fn free_space(path: &Path) -> Result<u64, CaptureError> {
    let failed = CaptureError::platform("узнать свободное место на диске");
    let Some(folder) = path.ancestors().find(|p| p.is_dir()) else {
        return Err(failed(format!("папки {} нет", path.display())));
    };
    let wide: Vec<u16> = folder.as_os_str().encode_wide().chain([0]).collect();
    let mut free = 0u64;
    // SAFETY: `wide` is a NUL-terminated path that lives through the call; the call writes
    // one u64 into `free`.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&raw mut free), None, None) }
        .map_err(|e| failed(e.to_string()))?;
    Ok(free)
}

/// Refuses to start a recording into `root` when its disk has less than [`ROOM_TO_START`].
pub fn room_to_start(root: &Path) -> Result<(), CaptureError> {
    enough(root, free_space(root)?)
}

fn enough(root: &Path, free: u64) -> Result<(), CaptureError> {
    match free < ROOM_TO_START {
        true => Err(CaptureError::DiskFull {
            folder: root.to_path_buf(),
            free_mb: free / MB,
            needed_mb: ROOM_TO_START / MB,
        }),
        false => Ok(()),
    }
}

/// What to tell the user of a running recording when `free` bytes are left; none while there
/// is room.
pub fn low_room(free: u64) -> Option<String> {
    (free < ROOM_LOW).then(|| {
        format!(
            "на диске осталось {} МБ: останови запись, пока место не кончилось, иначе её конец потеряется",
            free / MB
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disk_tells_its_free_room_also_for_a_folder_not_made_yet() {
        let temp = tempfile::tempdir().unwrap();
        let free = free_space(temp.path()).unwrap();
        assert!(free > 0);
        let inside = temp.path().join("Записи").join("recording-1.devcast");
        assert!(free_space(&inside).unwrap() > 0);
    }

    #[test]
    fn a_nearly_full_disk_refuses_a_recording_and_warns_a_running_one() {
        let root = Path::new("recording-1.devcast");
        assert!(enough(root, ROOM_TO_START).is_ok());
        let error = enough(root, 120 * MB).unwrap_err().to_string();
        assert!(
            error.contains("120 МБ") && error.contains("500 МБ"),
            "{error}"
        );
        assert_eq!(low_room(ROOM_LOW), None);
        assert!(low_room(150 * MB).unwrap().contains("150 МБ"));
    }
}
