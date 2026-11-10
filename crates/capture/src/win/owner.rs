//! The window of this app the user works in, as the owner of a system dialog: a file dialog
//! with an owner opens over it and stays above it; without one Windows puts the dialog on the
//! main screen, wherever the app's window is.

use std::num::NonZeroIsize;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    Win32WindowHandle, WindowHandle,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// A window of this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner(NonZeroIsize);

/// The foreground window if it is a window of this process: the one whose button asked for
/// the dialog.
pub fn foreground() -> Option<Owner> {
    let mut process = 0u32;
    // SAFETY: no pointers but `process`, where the call writes one u32; a handle that is not
    // a window leaves it 0.
    let window = unsafe {
        let window = GetForegroundWindow();
        GetWindowThreadProcessId(window, Some(&raw mut process));
        window
    };
    // SAFETY: the call takes nothing and only reads the id of this process.
    let ours = process == unsafe { GetCurrentProcessId() };
    NonZeroIsize::new(window.0 as isize)
        .filter(|_| ours)
        .map(Owner)
}

impl HasWindowHandle for Owner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
        // SAFETY: the handle is a window of this process that was in front a moment ago; a
        // window closed since is a handle Windows refuses, not memory.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for Owner {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_run_has_no_window_in_front_or_owns_it() {
        // A console test owns no window: the dialog then opens without an owner.
        if let Some(owner) = foreground() {
            assert!(owner.window_handle().is_ok() && owner.display_handle().is_ok());
        }
    }
}
