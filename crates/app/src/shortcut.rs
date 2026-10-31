//! A key combination by physical key: stored as text `Ctrl+Shift+Z` (modifiers in this order,
//! then the egui name of the key), so the Russian layout and any other give the same shortcut.

use dc_ui_kit::strings::keys as text;
use eframe::egui::{self, Key, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: Key,
}

impl Shortcut {
    /// `modifiers` with `key`; Windows has no Cmd, so egui's `command` is Ctrl.
    pub fn new(modifiers: Modifiers, key: Key) -> Self {
        Self {
            ctrl: modifiers.ctrl || modifiers.command,
            alt: modifiers.alt,
            shift: modifiers.shift,
            key,
        }
    }

    /// Reads `Ctrl+Shift+Z`; modifiers in any order and case.
    pub fn parse(stored: &str) -> Option<Self> {
        let mut parts: Vec<&str> = stored.split('+').map(str::trim).collect();
        let key = Key::from_name(parts.pop()?)?;
        let mut shortcut = Self::new(Modifiers::NONE, key);
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" => shortcut.ctrl = true,
                "alt" => shortcut.alt = true,
                "shift" => shortcut.shift = true,
                _ => return None,
            }
        }
        Some(shortcut)
    }

    fn modifiers(self) -> impl Iterator<Item = &'static str> {
        [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
    }

    /// The stored text.
    pub fn text(self) -> String {
        let mut parts: Vec<&str> = self.modifiers().collect();
        parts.push(self.key.name());
        parts.join("+")
    }

    /// The key caps to draw: `Ctrl`, `Shift`, `←`.
    pub fn caps(self) -> Vec<&'static str> {
        let key = text::cap(self.key.name()).unwrap_or_else(|| self.key.symbol_or_name());
        self.modifiers().chain([key]).collect()
    }

    /// The caps joined for a tooltip: `Ctrl+Z`, `Shift+Del`.
    pub fn label(self) -> String {
        self.caps().join("+")
    }

    /// The shortcut an input event presses, by physical key; `modifiers` are the frame's.
    ///
    /// egui-winit turns Ctrl+X, Shift+Delete (Windows), Ctrl+C, Ctrl+Insert, Ctrl+V and
    /// Shift+Insert into clipboard events without a key event; they are turned back here by the
    /// held modifiers (Ctrl+Insert reads as Ctrl+C: the system makes both «copy»).
    pub fn of_event(event: &egui::Event, modifiers: Modifiers) -> Option<Self> {
        let key = match event {
            egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } => return Some(Self::new(*modifiers, physical_key.unwrap_or(*key))),
            egui::Event::Cut if modifiers.shift => Key::Delete,
            egui::Event::Cut => Key::X,
            egui::Event::Copy => Key::C,
            egui::Event::Paste(_) if modifiers.shift && !Self::new(modifiers, Key::V).ctrl => {
                Key::Insert
            }
            egui::Event::Paste(_) => Key::V,
            _ => return None,
        };
        Some(Self::new(modifiers, key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl_shift() -> Modifiers {
        Modifiers::COMMAND | Modifiers::SHIFT
    }

    #[test]
    fn text_round_trips() {
        for stored in [
            "Ctrl+Shift+Z",
            "Space",
            "Shift+Delete",
            "OpenBracket",
            "F1",
            "Alt+Left",
        ] {
            let shortcut = Shortcut::parse(stored).unwrap();
            assert_eq!(shortcut.text(), stored);
        }
        assert_eq!(
            Shortcut::parse("shift+ctrl+z"),
            Shortcut::parse("Ctrl+Shift+Z")
        );
        assert_eq!(Shortcut::parse("Ctrl+Nothing"), None);
        assert_eq!(Shortcut::parse("Hyper+Z"), None);
    }

    #[test]
    fn caps_use_symbols() {
        let caps = |stored| Shortcut::parse(stored).unwrap().label();
        assert_eq!(caps("Shift+Delete"), "Shift+Del");
        assert_eq!(caps("Left"), "←");
        assert_eq!(caps("CloseBracket"), "]");
        assert_eq!(caps("Ctrl+Shift+Z"), "Ctrl+Shift+Z");
    }

    #[test]
    fn the_physical_key_decides() {
        // Russian «Я» has no egui key: egui-winit gives the physical Z.
        let event = egui::Event::Key {
            key: Key::Z,
            physical_key: Some(Key::Z),
            pressed: true,
            repeat: false,
            modifiers: ctrl_shift(),
        };
        let pressed = Shortcut::of_event(&event, ctrl_shift());
        assert_eq!(pressed, Shortcut::parse("Ctrl+Shift+Z"));
        // Dvorak: the letter differs, the place does not.
        let event = egui::Event::Key {
            key: Key::Semicolon,
            physical_key: Some(Key::Z),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            Shortcut::of_event(&event, Modifiers::NONE),
            Shortcut::parse("Z")
        );
        let released = egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(Shortcut::of_event(&released, Modifiers::NONE), None);
    }

    #[test]
    fn clipboard_events_turn_back_into_keys() {
        let of = |event, modifiers| Shortcut::of_event(&event, modifiers).map(Shortcut::text);
        assert_eq!(
            of(egui::Event::Cut, Modifiers::COMMAND).as_deref(),
            Some("Ctrl+X")
        );
        assert_eq!(
            of(egui::Event::Cut, Modifiers::SHIFT).as_deref(),
            Some("Shift+Delete")
        );
        assert_eq!(
            of(egui::Event::Copy, Modifiers::COMMAND).as_deref(),
            Some("Ctrl+C")
        );
        assert_eq!(
            of(egui::Event::Paste(String::new()), Modifiers::COMMAND).as_deref(),
            Some("Ctrl+V")
        );
        assert_eq!(
            of(egui::Event::Paste(String::new()), Modifiers::SHIFT).as_deref(),
            Some("Shift+Insert")
        );
    }
}
