//! The low-level keyboard hook procedure. It hands the key to the filter of the hook thread
//! at once; only the presses it returns are stamped and queued (decision 009).

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use super::hooks::send_key;
use crate::input::Stroke;

pub unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lparam points to a KBDLLHOOKSTRUCT that
        // the system owns for the duration of the call.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        send_key(Stroke {
            // Virtual-key codes are 1..=254.
            key: info.vkCode as u16,
            scan: info.scanCode,
            down: matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN),
        });
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
