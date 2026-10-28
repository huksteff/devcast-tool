//! `keys.bin`: shortcuts pressed during a recording, and the privacy filter that decides what
//! is a shortcut (decision 009). Typed text never passes the filter: it runs inside the hook,
//! before anything is queued or written.
//!
//! Record, 12 bytes little-endian: time (i64 µs on the host clock), key (u16, Windows
//! virtual-key code), modifiers (u8: 1 Ctrl, 2 Shift, 4 Alt, 8 Win), 0.

use std::path::Path;

use super::file::{Record, Timed, read_records};
use crate::error::CaptureError;
use crate::event::HostTime;

/// File name in the recording folder.
pub const KEYS_FILE: &str = "keys.bin";

/// Modifiers held when a key went down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

/// A key pressed as part of a shortcut: `key` is a Windows virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: u16,
    pub modifiers: Modifiers,
}

/// Keys kept even without Ctrl, Alt or Win: Backspace, Tab, Enter, Esc, Page Up/Down, End,
/// Home, the arrows, Print Screen, Insert, Delete, F1–F24.
const SPECIAL: [(u16, u16); 6] = [
    (0x08, 0x09),
    (0x0D, 0x0D),
    (0x1B, 0x1B),
    (0x21, 0x28),
    (0x2C, 0x2E),
    (0x70, 0x87),
];

/// Modifier keys (generic, left, right) and the modifier each one is.
const MODIFIER_KEYS: [(u16, usize); 11] = [
    (0x11, 0), // VK_CONTROL
    (0xA2, 0), // VK_LCONTROL
    (0xA3, 0), // VK_RCONTROL
    (0x10, 1), // VK_SHIFT
    (0xA0, 1), // VK_LSHIFT
    (0xA1, 1), // VK_RSHIFT
    (0x12, 2), // VK_MENU
    (0xA4, 2), // VK_LMENU
    (0xA5, 2), // VK_RMENU
    (0x5B, 3), // VK_LWIN
    (0x5C, 3), // VK_RWIN
];

fn is_special(key: u16) -> bool {
    SPECIAL
        .iter()
        .any(|&(first, last)| (first..=last).contains(&key))
}

/// The privacy filter of the keyboard hook. It keeps which modifier keys are down itself:
/// in a low-level hook `GetKeyState` is not updated yet.
#[derive(Debug, Default)]
pub struct KeyFilter {
    /// Modifier keys down, one bit per entry of `MODIFIER_KEYS`.
    down: u16,
}

impl KeyFilter {
    /// The key `key` went down (`true`) or up. Returns the event to record: a key pressed with
    /// Ctrl, Alt or Win held, or a special key. Anything else — letters, digits, Shift+letter,
    /// modifiers alone — gives `None` and is forgotten.
    pub fn key(&mut self, key: u16, down: bool) -> Option<KeyEvent> {
        if let Some(bit) = MODIFIER_KEYS.iter().position(|&(k, _)| k == key) {
            match down {
                true => self.down |= 1 << bit,
                false => self.down &= !(1 << bit),
            }
            return None;
        }
        let modifiers = self.modifiers();
        let shortcut = modifiers.ctrl || modifiers.alt || modifiers.win || is_special(key);
        (down && shortcut).then_some(KeyEvent { key, modifiers })
    }

    fn modifiers(&self) -> Modifiers {
        let held = |modifier: usize| {
            MODIFIER_KEYS
                .iter()
                .enumerate()
                .any(|(bit, &(_, m))| m == modifier && self.down & (1 << bit) != 0)
        };
        Modifiers {
            ctrl: held(0),
            shift: held(1),
            alt: held(2),
            win: held(3),
        }
    }
}

impl Modifiers {
    fn bits(self) -> u8 {
        u8::from(self.ctrl)
            | u8::from(self.shift) << 1
            | u8::from(self.alt) << 2
            | u8::from(self.win) << 3
    }

    fn from_bits(bits: u8) -> Result<Self, String> {
        if bits > 0xF {
            return Err(format!("неизвестные модификаторы {bits:#x}"));
        }
        Ok(Self {
            ctrl: bits & 1 != 0,
            shift: bits & 2 != 0,
            alt: bits & 4 != 0,
            win: bits & 8 != 0,
        })
    }
}

impl Record for Timed<KeyEvent> {
    const MAGIC: [u8; 4] = *b"DCKY";
    const VERSION: u16 = 1;
    const SIZE: usize = 12;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.at.micros().to_le_bytes());
        out.extend_from_slice(&self.event.key.to_le_bytes());
        out.extend_from_slice(&[self.event.modifiers.bits(), 0]);
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut time = [0; 8];
        time.copy_from_slice(&bytes[..8]);
        Ok(Self {
            at: HostTime::from_micros(i64::from_le_bytes(time)),
            event: KeyEvent {
                key: u16::from_le_bytes([bytes[8], bytes[9]]),
                modifiers: Modifiers::from_bits(bytes[10])?,
            },
        })
    }
}

/// Shortcuts of the recording in folder `dir`, in time order.
pub fn read_keys(dir: &Path) -> Result<Vec<Timed<KeyEvent>>, CaptureError> {
    read_records(&dir.join(KEYS_FILE))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
