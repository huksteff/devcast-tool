//! Shortcuts in the windows (`dc_settings::Shortcut` is the stored one): the key caps to draw and
//! the shortcut an input event presses.

use dc_settings::Shortcut;
use dc_ui_kit::strings::keys as text;
use eframe::egui::{self, Key, Modifiers};

pub trait Caps: Sized {
    /// The key caps to draw: `Ctrl`, `Shift`, `←`.
    fn caps(self) -> Vec<&'static str>;

    /// The caps joined for a tooltip: `Ctrl+Z`, `Shift+Del`.
    fn label(self) -> String;

    /// The shortcut an input event presses, by physical key; `modifiers` are the frame's.
    fn of_event(event: &egui::Event, modifiers: Modifiers) -> Option<Self>;
}

impl Caps for Shortcut {
    fn caps(self) -> Vec<&'static str> {
        let key = text::cap(self.key.name()).unwrap_or_else(|| self.key.symbol_or_name());
        self.modifiers().chain([key]).collect()
    }

    fn label(self) -> String {
        self.caps().join("+")
    }

    /// egui-winit turns Ctrl+X, Shift+Delete (Windows), Ctrl+C, Ctrl+Insert, Ctrl+V and
    /// Shift+Insert into clipboard events without a key event; they are turned back here by the
    /// held modifiers (Ctrl+Insert reads as Ctrl+C: the system makes both «copy»).
    fn of_event(event: &egui::Event, modifiers: Modifiers) -> Option<Self> {
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
