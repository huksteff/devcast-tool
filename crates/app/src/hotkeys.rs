//! Global hotkeys (global-hotkey, created on the event loop thread like the tray): start/stop,
//! pause, cancel, from any program and with the HUD hidden. Shortcuts are stored as text
//! (`Ctrl+Shift+R`, Latin names of physical keys, so a Russian layout gives the same). A
//! shortcut the system or another program holds fails to register: the settings show it.

use std::str::FromStr;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};

use dc_ui_kit::strings::hotkeys as text;
use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::error::AppError;
use crate::settings::Hotkeys;

/// What a global hotkey does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    StartStop,
    Pause,
    Cancel,
}

impl Command {
    pub const ALL: [Self; 3] = [Self::StartStop, Self::Pause, Self::Cancel];

    pub fn title(self) -> &'static str {
        match self {
            Self::StartStop => text::START_STOP,
            Self::Pause => text::PAUSE,
            Self::Cancel => text::CANCEL,
        }
    }

    pub fn keys(self, hotkeys: &Hotkeys) -> &Option<String> {
        match self {
            Self::StartStop => &hotkeys.start_stop,
            Self::Pause => &hotkeys.pause,
            Self::Cancel => &hotkeys.cancel,
        }
    }

    pub fn keys_mut(self, hotkeys: &mut Hotkeys) -> &mut Option<String> {
        match self {
            Self::StartStop => &mut hotkeys.start_stop,
            Self::Pause => &mut hotkeys.pause,
            Self::Cancel => &mut hotkeys.cancel,
        }
    }
}

/// The shortcut of a key press as text, or `None` without Ctrl or Alt (a global hotkey needs
/// one). `key` is the physical key: the Latin name works in any layout.
pub fn combo(modifiers: egui::Modifiers, key: egui::Key) -> Option<String> {
    if !(modifiers.ctrl || modifiers.alt) {
        return None;
    }
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    parts.push(key.name());
    Some(parts.join("+"))
}

/// Another command that already has `keys`.
pub fn taken_by(hotkeys: &Hotkeys, keys: &str, except: Command) -> Option<Command> {
    Command::ALL
        .into_iter()
        .filter(|c| *c != except)
        .find(|c| c.keys(hotkeys).as_deref() == Some(keys))
}

type Bound = Arc<Mutex<Vec<(u32, Command)>>>;

/// The registered hotkeys and their presses.
pub struct GlobalKeys {
    manager: GlobalHotKeyManager,
    bound: Bound,
    registered: Vec<HotKey>,
    presses: Receiver<Command>,
    /// The shortcuts that are applied now.
    applied: Option<Hotkeys>,
}

impl GlobalKeys {
    pub fn new(ctx: &egui::Context) -> Result<Self, AppError> {
        let manager = GlobalHotKeyManager::new().map_err(|e| AppError::Hotkeys(e.to_string()))?;
        let bound: Bound = Arc::default();
        let (sender, presses) = mpsc::channel();
        let (ids, ctx) = (Arc::clone(&bound), ctx.clone());
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() != HotKeyState::Pressed {
                return;
            }
            let ids = ids.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((_, command)) = ids.iter().find(|(id, _)| *id == event.id()) {
                // The receiver goes only with the app.
                let _ = sender.send(*command);
                ctx.request_repaint();
            }
        }));
        Ok(Self {
            manager,
            bound,
            registered: Vec::new(),
            presses,
            applied: None,
        })
    }

    pub fn presses(&self) -> Vec<Command> {
        self.presses.try_iter().collect()
    }

    /// Registers `hotkeys` when they differ from the applied ones; returns `None` when nothing
    /// changed, else the commands whose shortcut the system did not give us.
    pub fn apply(&mut self, hotkeys: &Hotkeys) -> Option<Vec<Command>> {
        if self.applied.as_ref() == Some(hotkeys) {
            return None;
        }
        if let Err(error) = self.manager.unregister_all(&self.registered) {
            tracing::warn!("старые горячие клавиши не сняты: {error}");
        }
        self.registered.clear();
        let mut bound = Vec::new();
        let mut taken = Vec::new();
        for command in Command::ALL {
            let Some(keys) = command.keys(hotkeys) else {
                continue;
            };
            let registered = HotKey::from_str(keys)
                .map_err(|e| e.to_string())
                .and_then(|key| {
                    self.manager
                        .register(key)
                        .map(|()| key)
                        .map_err(|e| e.to_string())
                });
            match registered {
                Ok(key) => {
                    bound.push((key.id(), command));
                    self.registered.push(key);
                }
                Err(error) => {
                    tracing::info!("{keys} не зарегистрировано: {error}");
                    taken.push(command);
                }
            }
        }
        *self.bound.lock().unwrap_or_else(PoisonError::into_inner) = bound;
        self.applied = Some(hotkeys.clone());
        Some(taken)
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};

    use super::*;

    #[test]
    fn shortcut_needs_ctrl_or_alt() {
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(combo(ctrl_shift, Key::R).as_deref(), Some("Ctrl+Shift+R"));
        assert_eq!(combo(Modifiers::SHIFT, Key::R), None);
        assert_eq!(combo(Modifiers::ALT, Key::F5).as_deref(), Some("Alt+F5"));
    }

    #[test]
    fn stored_shortcuts_parse_for_registration() {
        for keys in [
            "Ctrl+Shift+R",
            "Ctrl+Shift+P",
            "Ctrl+Shift+X",
            "Alt+F5",
            "Ctrl+Alt+1",
        ] {
            assert!(HotKey::from_str(keys).is_ok(), "{keys}");
        }
    }

    #[test]
    fn a_shortcut_of_another_command_is_found() {
        let hotkeys = Hotkeys::default();
        assert_eq!(
            taken_by(&hotkeys, "Ctrl+Shift+R", Command::Cancel),
            Some(Command::StartStop)
        );
        assert_eq!(taken_by(&hotkeys, "Ctrl+Shift+R", Command::StartStop), None);
    }
}
