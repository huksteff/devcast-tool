//! Synthetic mouse and keyboard input (SendInput) and putting the pointer back.

#![allow(
    unsafe_code,
    reason = "SendInput and the pointer position are Win32 calls"
)]

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MAPVK_VK_TO_CHAR, MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, MapVirtualKeyExW, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetPhysicalCursorPos, GetSystemMetrics, GetWindowThreadProcessId,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SetPhysicalCursorPos,
};

/// Puts the pointer back where it was, also on panic.
pub struct PointerBack(POINT);

impl PointerBack {
    pub fn save() -> Self {
        let mut point = POINT::default();
        // SAFETY: writes the pointer position into `point`.
        unsafe { GetPhysicalCursorPos(&mut point) }.unwrap();
        Self(point)
    }
}

impl Drop for PointerBack {
    fn drop(&mut self) {
        // SAFETY: moves the pointer to where it was.
        let _ = unsafe { SetPhysicalCursorPos(self.0.x, self.0.y) };
    }
}

/// Sends `inputs` in one `SendInput`; nothing of it may be blocked.
pub fn send(inputs: &[INPUT]) {
    // SAFETY: a slice of initialized INPUT structures and their size.
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    assert_eq!(sent as usize, inputs.len(), "input was blocked");
}

pub fn mouse(flags: MOUSE_EVENT_FLAGS, x: i32, y: i32, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: x,
                dy: y,
                mouseData: data as u32,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

/// A move to desktop pixel (`x`, `y`) in the 0..65535 units of the virtual desktop.
pub fn move_input(x: i32, y: i32) -> INPUT {
    // SAFETY: reads system metrics.
    let (left, top, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let scale = |v: i32, origin: i32, size: i32| ((v - origin) * 65535 + size / 2) / (size - 1);
    let flags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    mouse(flags, scale(x, left, width), scale(y, top, height), 0)
}

pub fn key(vk: u16, up: bool) -> INPUT {
    let flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

/// `keys` pressed in order and released in reverse.
pub fn chord_inputs(keys: &[u16]) -> Vec<INPUT> {
    let downs = keys.iter().map(|&k| key(k, false));
    let ups = keys.iter().rev().map(|&k| key(k, true));
    downs.chain(ups).collect()
}

/// What typing the ASCII `text` key by key gives in the keyboard layout of the foreground
/// window: synthetic keys are virtual-key codes, and `A` types «ф» in a Russian layout. The
/// layout is the user's setting, so tests compare with this instead of switching it.
pub fn as_typed(text: &str) -> String {
    // SAFETY: reads the foreground window, its thread and that thread's keyboard layout.
    let layout = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        GetKeyboardLayout(thread)
    };
    text.chars()
        .map(|c| {
            if !c.is_ascii_alphabetic() {
                return c;
            }
            let vk = u32::from(c.to_ascii_uppercase());
            // SAFETY: a pure lookup in the given layout.
            let mapped = unsafe { MapVirtualKeyExW(vk, MAPVK_VK_TO_CHAR, Some(layout)) };
            let base = char::from_u32(mapped & 0xFFFF).unwrap_or(c);
            if c.is_ascii_uppercase() {
                base.to_uppercase().next().unwrap_or(base)
            } else {
                base.to_lowercase().next().unwrap_or(base)
            }
        })
        .collect()
}
