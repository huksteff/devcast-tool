//! `keys.bin`: every key pressed during a recording, typed text too (decision 009, changed by
//! the owner 2026-11-04), and the filter of the keyboard hook that turns key strokes into
//! presses.
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

/// A key going down or up, as the keyboard hook reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    /// Windows virtual-key code.
    pub key: u16,
    /// Hardware scan code.
    pub scan: u32,
    pub down: bool,
}

/// A key pressed with the modifiers held: `key` is a Windows virtual-key code. A modifier tapped
/// alone is a press of its own key (`Ctrl`, `Ctrl+Shift`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: u16,
    pub modifiers: Modifiers,
}

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

const LCONTROL: u16 = 0xA2;
const RMENU: u16 = 0xA5;

/// The scan code of the left Ctrl that Windows adds itself when AltGr (right Alt on layouts
/// where it types characters) goes down or up.
const ALTGR_CONTROL_SCAN: u32 = 0x21D;

/// The filter of the keyboard hook. It keeps which modifier keys are down itself: in a
/// low-level hook `GetKeyState` is not updated yet.
#[derive(Debug, Default)]
pub struct KeyFilter {
    /// Modifier keys down, one bit per entry of `MODIFIER_KEYS`.
    down: u16,
    /// The left Ctrl that Windows adds for AltGr is down: the right Alt is AltGr, it types
    /// characters and is not a modifier.
    altgr: bool,
    /// Only modifiers went down since the last other key: the first of them released is a press
    /// of the modifiers held (`Ctrl+Shift`), whatever order they are released in.
    tapped: bool,
}

impl KeyFilter {
    /// A key went down or up. Returns the press to record: any other key going down, with the
    /// modifiers held, or a modifier released with no other key pressed since it went down.
    /// The left Ctrl Windows adds for AltGr is not a key of its own and gives `None`.
    pub fn key(&mut self, stroke: Stroke) -> Option<KeyEvent> {
        let Stroke { key, down, .. } = stroke;
        if key == LCONTROL && stroke.scan == ALTGR_CONTROL_SCAN {
            self.altgr = down;
            return None;
        }
        if let Some(bit) = MODIFIER_KEYS.iter().position(|&(k, _)| k == key) {
            return self.modifier(bit, key, down);
        }
        self.tapped = false;
        down.then(|| KeyEvent {
            key,
            modifiers: self.modifiers(),
        })
    }

    fn modifier(&mut self, bit: usize, key: u16, down: bool) -> Option<KeyEvent> {
        if down {
            // Auto-repeat of a held modifier keeps its tap.
            if self.down & (1 << bit) == 0 {
                self.tapped = true;
            }
            self.down |= 1 << bit;
            return None;
        }
        let was_down = self.down & (1 << bit) != 0;
        self.down &= !(1 << bit);
        (was_down && std::mem::take(&mut self.tapped)).then(|| KeyEvent {
            key,
            modifiers: self.modifiers(),
        })
    }

    fn modifiers(&self) -> Modifiers {
        let held = |modifier: usize| {
            MODIFIER_KEYS
                .iter()
                .enumerate()
                .filter(|&(_, &(k, _))| !(self.altgr && k == RMENU))
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
    /// These modifiers plus the one `key` is, if it is a modifier key: a tapped `Ctrl` reads as
    /// the modifier, not as a key after it.
    pub(crate) fn with_key(self, key: u16) -> (Self, bool) {
        let Some(&(_, modifier)) = MODIFIER_KEYS.iter().find(|&&(k, _)| k == key) else {
            return (self, false);
        };
        let mut all = self;
        match modifier {
            0 => all.ctrl = true,
            1 => all.shift = true,
            2 => all.alt = true,
            _ => all.win = true,
        }
        (all, true)
    }

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

/// Key presses of the recording in folder `dir`, in time order.
pub fn read_keys(dir: &Path) -> Result<Vec<Timed<KeyEvent>>, CaptureError> {
    read_records(&dir.join(KEYS_FILE))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
