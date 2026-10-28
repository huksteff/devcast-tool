//! Synthetic mouse and keyboard (SendInput) for the fixtures. The pointer goes only over the
//! canvas on the test monitor and is put back at the end.

use std::time::Duration;

pub use crate::common::synthetic::{PointerBack, as_typed};
use crate::common::synthetic::{chord_inputs, key, mouse, move_input, send};
use windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS;
pub use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL,
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

pub fn wait(millis: u64) {
    std::thread::sleep(Duration::from_millis(millis));
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

/// Presses `keys` in order and releases them in reverse.
pub fn chord(keys: &[u16]) {
    send(&chord_inputs(keys));
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
