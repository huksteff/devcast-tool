//! Global hotkeys (global-hotkey, created on the event loop thread like the tray): start/stop,
//! pause, cancel, from any program and with the HUD hidden. Shortcuts are stored as text
//! (`Ctrl+Shift+R`, Latin names of physical keys, so a Russian layout gives the same). A
//! shortcut the system or another program holds fails to register: the settings show it.

use std::str::FromStr;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::error::AppError;
pub use crate::keymap::Command;
use crate::settings::Hotkeys;

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
                ctx.request_repaint_of(egui::ViewportId::ROOT);
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
    use super::*;

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
}
