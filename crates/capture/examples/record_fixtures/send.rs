//! Synthetic mouse and keyboard (SendInput) for the fixtures. The pointer goes only over the
//! canvas on the test monitor and is put back at the end.

use std::time::Duration;

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
    SendInput, VIRTUAL_KEY,
};
pub use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetPhysicalCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetPhysicalCursorPos,
};

pub const CTRL: u16 = 0x11;
pub const SHIFT: u16 = 0x10;
pub const ALT: u16 = 0x12;
pub const ENTER: u16 = 0x0D;
pub const ESC: u16 = 0x1B;
pub const SPACE: u16 = 0x20;
pub const LEFT: u16 = 0x25;
pub const DELETE: u16 = 0x2E;
pub const F5: u16 = 0x74;

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

pub fn wait(millis: u64) {
    std::thread::sleep(Duration::from_millis(millis));
}

fn send(inputs: &[INPUT]) {
    // SAFETY: a slice of initialized INPUT structures and their size.
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    assert_eq!(sent as usize, inputs.len(), "input was blocked");
}

fn mouse(flags: MOUSE_EVENT_FLAGS, x: i32, y: i32, data: i32) -> INPUT {
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
fn move_input(x: i32, y: i32) -> INPUT {
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

pub fn move_to(x: i32, y: i32) {
    send(&[move_input(x, y)]);
}

/// Moves from (`x0`, `y0`) to (`x1`, `y1`) in `steps` moves, `millis` apart.
pub fn glide((x0, y0): (i32, i32), (x1, y1): (i32, i32), steps: i32, millis: u64) {
    for i in 1..=steps {
        move_to(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
        wait(millis);
    }
}

/// A button or wheel action at desktop pixel (`x`, `y`), the move and the action in one
/// `SendInput`.
pub fn act(x: i32, y: i32, flags: MOUSE_EVENT_FLAGS, data: i32) {
    send(&[move_input(x, y), mouse(flags, 0, 0, data)]);
}

pub fn click(x: i32, y: i32) {
    act(x, y, MOUSEEVENTF_LEFTDOWN, 0);
    wait(60);
    act(x, y, MOUSEEVENTF_LEFTUP, 0);
}

fn key(vk: u16, up: bool) -> INPUT {
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

/// Presses `keys` in order and releases them in reverse.
pub fn chord(keys: &[u16]) {
    let downs = keys.iter().map(|&k| key(k, false));
    let ups = keys.iter().rev().map(|&k| key(k, true));
    send(&downs.chain(ups).collect::<Vec<_>>());
    wait(150);
}

/// Types lowercase Latin letters and spaces, one key at a time, like a person.
pub fn type_text(text: &str) {
    for c in text.chars() {
        let vk = match c {
            ' ' => SPACE,
            c => u16::from(c.to_ascii_uppercase() as u8),
        };
        send(&[key(vk, false), key(vk, true)]);
        wait(90);
    }
}
