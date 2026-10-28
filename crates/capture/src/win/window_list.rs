//! Windows a user would pick: windows-capture's list (visible, not tool, not child windows)
//! without what the taskbar does not show either.

use std::path::Path;

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, IsIconic};
use windows::core::{Owned, PWSTR};
use windows_capture::window::Window;

use crate::error::CaptureError;
use crate::list::WindowInfo;

pub fn list() -> Result<Vec<WindowInfo>, CaptureError> {
    let failed = |e: windows_capture::window::Error| {
        CaptureError::platform("получить список окон")(e.to_string())
    };
    // SAFETY: plain queries without arguments.
    let (own_process, shell) = unsafe { (GetCurrentProcessId(), GetShellWindow()) };
    let mut found = Vec::new();
    for window in Window::enumerate().map_err(failed)? {
        let hwnd = HWND(window.as_raw_hwnd());
        // A window that closed during the listing has no process; it is simply not listed.
        let Ok(process) = window.process_id() else {
            continue;
        };
        let traits = Traits {
            title: window.title().map_err(failed)?,
            cloaked: is_cloaked(hwnd),
            own_process: process == own_process,
            shell: hwnd == shell,
        };
        if traits.pickable() {
            found.push(WindowInfo {
                id: hwnd.0 as u64,
                title: traits.title,
                app: app_name(process),
                // SAFETY: IsIconic accepts any handle and returns false for invalid ones.
                minimized: unsafe { IsIconic(hwnd) }.as_bool(),
            });
        }
    }
    Ok(found)
}

/// What decides whether a listed window is offered to the user.
struct Traits {
    title: String,
    /// Hidden by DWM: suspended store apps, windows on other virtual desktops.
    cloaked: bool,
    /// Our own windows (HUD, editor) are never recorded.
    own_process: bool,
    /// The desktop itself (Program Manager).
    shell: bool,
}

impl Traits {
    fn pickable(&self) -> bool {
        !self.title.trim().is_empty() && !self.cloaked && !self.own_process && !self.shell
    }
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: DWMWA_CLOAKED writes one u32 into `cloaked`, whose size is passed.
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked != 0
}

/// Program file name without extension; `None` when the system does not tell it
/// (protected processes).
fn app_name(process: u32) -> Option<String> {
    // SAFETY: the handle is owned and closed by `Owned`; the buffer length is passed in `len`.
    unsafe {
        let handle: Owned<HANDLE> =
            Owned::new(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process).ok()?);
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        QueryFullProcessImageNameW(
            *handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        )
        .ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        Some(Path::new(&path).file_stem()?.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traits(title: &str) -> Traits {
        Traits {
            title: title.to_owned(),
            cloaked: false,
            own_process: false,
            shell: false,
        }
    }

    #[test]
    fn only_titled_foreign_visible_windows() {
        assert!(traits("Документ — Блокнот").pickable());
        assert!(!traits("").pickable());
        assert!(!traits("  ").pickable());
        assert!(
            !Traits {
                cloaked: true,
                ..traits("Calculator")
            }
            .pickable()
        );
        assert!(
            !Traits {
                own_process: true,
                ..traits("DevCast")
            }
            .pickable()
        );
        assert!(
            !Traits {
                shell: true,
                ..traits("Program Manager")
            }
            .pickable()
        );
    }
}
