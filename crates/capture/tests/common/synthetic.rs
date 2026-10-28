//! Synthetic mouse and keyboard input (SendInput) and putting the pointer back.

#![allow(
    unsafe_code,
    reason = "SendInput and the pointer position are Win32 calls"
)]

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
    SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetPhysicalCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetPhysicalCursorPos,
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

/// Typed text with every letter as `a` or `A` and control characters left out: synthetic keys
/// are virtual-key codes, so `A` types «ф» in a Russian layout, and the layout is the user's
/// setting. Tests compare shapes, which tell that every key arrived in order.
pub fn letter_shape(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\r')
        .map(|c| match c {
            c if c.is_uppercase() => 'A',
            c if c.is_alphabetic() => 'a',
            c => c,
        })
        .collect()
}
