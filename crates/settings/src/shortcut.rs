//! A key combination by physical key: stored as text `Ctrl+Shift+Z` (modifiers in this order,
//! then the egui name of the key), so the Russian layout and any other give the same shortcut.
//! Windows has no Cmd and the Win key is the system's: there is no modifier for them.

use egui::{Key, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
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

    /// The names of the held modifiers, in the stored order.
    pub fn modifiers(self) -> impl Iterator<Item = &'static str> {
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
}

impl TryFrom<String> for Shortcut {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| format!("сочетание «{text}» не читается"))
    }
}

impl From<Shortcut> for String {
    fn from(shortcut: Shortcut) -> Self {
        shortcut.text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(Shortcut::parse("Win+R"), None);
        let json = serde_json::to_string(&Shortcut::parse("ctrl+b").unwrap()).unwrap();
        assert_eq!(json, r#""Ctrl+B""#);
    }
}
