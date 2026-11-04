//! Readable names of recorded key presses: `Ctrl+Shift+P`, `Alt+Tab`, `F5`, `A`, `Ctrl`.

use super::keys::KeyEvent;

/// Names of keys that are not a letter, digit or F-key, by Windows virtual-key code.
const NAMES: [(u16, &str); 37] = [
    (0x08, "Backspace"),
    (0x09, "Tab"),
    (0x0D, "Enter"),
    (0x13, "Pause"),
    (0x1B, "Esc"),
    (0x20, "Space"),
    (0x21, "PageUp"),
    (0x22, "PageDown"),
    (0x23, "End"),
    (0x24, "Home"),
    (0x25, "Left"),
    (0x26, "Up"),
    (0x27, "Right"),
    (0x28, "Down"),
    (0x2C, "PrintScreen"),
    (0x2D, "Insert"),
    (0x2E, "Delete"),
    (0x6A, "Num*"),
    (0x6B, "Num+"),
    (0x6D, "Num-"),
    (0x6E, "Num."),
    (0x6F, "Num/"),
    (0xBA, ";"),
    (0xBB, "="),
    (0xBC, ","),
    (0xBD, "-"),
    (0xBE, "."),
    (0xBF, "/"),
    (0xC0, "`"),
    (0xDB, "["),
    (0xDC, "\\"),
    (0xDD, "]"),
    (0xDE, "'"),
    (0x5D, "Menu"),
    (0x14, "CapsLock"),
    (0x90, "NumLock"),
    (0x91, "ScrollLock"),
];

/// The name of key `code` alone; an unknown key is `VK_<hex code>`.
fn key(code: u16) -> String {
    match code {
        0x30..=0x39 | 0x41..=0x5A => char::from(code as u8).to_string(),
        0x60..=0x69 => format!("Num{}", code - 0x60),
        0x70..=0x87 => format!("F{}", code - 0x6F),
        _ => NAMES
            .iter()
            .find(|(c, _)| *c == code)
            .map_or_else(|| format!("VK_{code:02X}"), |(_, name)| (*name).to_owned()),
    }
}

impl KeyEvent {
    /// Modifiers in the order Ctrl, Alt, Shift, Win, then the key, joined by `+`; a tapped
    /// modifier is only the modifiers (`Ctrl+Shift`).
    pub fn name(&self) -> String {
        let (m, modifier) = self.modifiers.with_key(self.key);
        let held = [
            (m.ctrl, "Ctrl"),
            (m.alt, "Alt"),
            (m.shift, "Shift"),
            (m.win, "Win"),
        ];
        let mut parts: Vec<String> = held
            .iter()
            .filter(|(down, _)| *down)
            .map(|(_, name)| (*name).to_owned())
            .collect();
        if !modifier {
            parts.push(key(self.key));
        }
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;

    fn name(key: u16, ctrl: bool, shift: bool, alt: bool, win: bool) -> String {
        let modifiers = Modifiers {
            ctrl,
            shift,
            alt,
            win,
        };
        KeyEvent { key, modifiers }.name()
    }

    #[test]
    fn shortcuts_read_like_on_the_keyboard() {
        assert_eq!(
            name(u16::from(b'P'), true, true, false, false),
            "Ctrl+Shift+P"
        );
        assert_eq!(name(0x09, false, false, true, false), "Alt+Tab");
        assert_eq!(name(0x74, false, false, false, false), "F5");
        assert_eq!(name(0x87, false, false, false, false), "F24");
        assert_eq!(name(u16::from(b'D'), false, false, false, true), "Win+D");
        assert_eq!(name(0x2E, true, false, true, false), "Ctrl+Alt+Delete");
        assert_eq!(name(0x31, true, false, false, false), "Ctrl+1");
        assert_eq!(name(0x65, true, false, false, false), "Ctrl+Num5");
        assert_eq!(name(0xBF, true, false, false, false), "Ctrl+/");
        assert_eq!(name(0xE7, true, false, false, false), "Ctrl+VK_E7");
        assert_eq!(name(u16::from(b'A'), false, false, false, false), "A");
        assert_eq!(name(0x20, false, true, false, false), "Shift+Space");
        assert_eq!(name(0xA2, false, false, false, false), "Ctrl");
        assert_eq!(name(0xA0, true, false, false, false), "Ctrl+Shift");
        assert_eq!(name(0xA3, false, true, false, false), "Ctrl+Shift");
        assert_eq!(name(0x5B, false, false, false, false), "Win");
    }
}
