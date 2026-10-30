//! The tray icon (mockup 6): the system menu of Windows with the status line, «Остановить»,
//! «Показать панель», «Настройки», «Выход». A left click shows the HUD, the right one the menu
//! (a stray click must not end a recording). The icon is the logo at rest and a red dot while
//! recording. Its events come through the tray-icon handlers and wake the UI.

use std::sync::mpsc::{self, Receiver};

use dc_ui_kit::strings::tray as text;
use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::error::AppError;

/// What the tray asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowHud,
    Stop,
    Settings,
    Quit,
}

/// What the menu shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayView {
    /// The first, inactive line: «DevCast — готов к записи», «● Запись 12:47 · Экран 1».
    pub status: String,
    /// A recording runs: red icon, «Остановить» first.
    pub recording: bool,
    /// «Остановить» works (not at rest and not while the recording is prepared).
    pub can_stop: bool,
}

pub struct Tray {
    icon: TrayIcon,
    menu: Menu,
    status: MenuItem,
    stop: MenuItem,
    show: MenuItem,
    actions: Receiver<TrayAction>,
    shown: Option<TrayView>,
}

/// Side of the icon, pixels.
const SIDE: u32 = 32;

impl Tray {
    pub fn new(ctx: &egui::Context) -> Result<Self, AppError> {
        let failed = |e: &dyn std::fmt::Display| AppError::Tray(e.to_string());
        let status = MenuItem::new(text::READY, false, None);
        let stop = MenuItem::new(text::STOP, false, None);
        let show = MenuItem::new(text::SHOW, true, None);
        let settings = MenuItem::new(text::SETTINGS, true, None);
        let quit = MenuItem::new(text::QUIT, true, None);
        let menu = Menu::with_items(&[
            &status,
            &PredefinedMenuItem::separator(),
            &show,
            &stop,
            &PredefinedMenuItem::separator(),
            &settings,
            &quit,
        ])
        .map_err(|e| failed(&e))?;
        let (sender, actions) = mpsc::channel();
        let ids = [
            (stop.id().clone(), TrayAction::Stop),
            (show.id().clone(), TrayAction::ShowHud),
            (settings.id().clone(), TrayAction::Settings),
            (quit.id().clone(), TrayAction::Quit),
        ];
        let (menu_sender, menu_ctx) = (sender.clone(), ctx.clone());
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, action)) = ids.iter().find(|(id, _)| *id == event.id) {
                // The receiver goes only with the app.
                let _ = menu_sender.send(*action);
                menu_ctx.request_repaint();
            }
        }));
        let click_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = sender.send(TrayAction::ShowHud);
                click_ctx.request_repaint();
            }
        }));
        let icon = TrayIconBuilder::new()
            .with_tooltip(text::TOOLTIP)
            .with_icon(picture(false).map_err(|e| failed(&e))?)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| failed(&e))?;
        Ok(Self {
            icon,
            menu,
            status,
            stop,
            show,
            actions,
            shown: None,
        })
    }

    pub fn actions(&self) -> Vec<TrayAction> {
        self.actions.try_iter().collect()
    }

    /// Brings the icon and the menu to `view`; nothing happens when it is already shown.
    pub fn sync(&mut self, view: TrayView) -> Result<(), AppError> {
        if self.shown.as_ref() == Some(&view) {
            return Ok(());
        }
        let was_recording = self.shown.as_ref().map(|v| v.recording);
        self.status.set_text(&view.status);
        self.stop.set_enabled(view.can_stop);
        if was_recording != Some(view.recording) {
            self.icon
                .set_icon(Some(
                    picture(view.recording).map_err(|e| AppError::Tray(e.to_string()))?,
                ))
                .map_err(|e| AppError::Tray(e.to_string()))?;
            // At rest «Показать панель» comes first, while recording «Остановить» (mockup 6).
            let first = if view.recording {
                &self.stop
            } else {
                &self.show
            };
            self.menu
                .remove(first)
                .and_then(|()| self.menu.insert(first, 2))
                .map_err(|e| AppError::Tray(e.to_string()))?;
        }
        self.shown = Some(view);
        Ok(())
    }
}

/// The icon: a blue disc with a white ▶ at rest (mockup), a red dot while recording.
fn picture(recording: bool) -> Result<Icon, tray_icon::BadIcon> {
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let c = SIDE as f32 / 2.0;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let inside = (dx * dx + dy * dy).sqrt() <= c - 1.0;
            let pixel = if !inside {
                [0, 0, 0, 0]
            } else if recording {
                [0xf4, 0x3f, 0x5e, 0xff]
            } else if play(dx, dy, c) {
                [0xff, 0xff, 0xff, 0xff]
            } else {
                [0x25, 0x63, 0xeb, 0xff]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    Icon::from_rgba(rgba, SIDE, SIDE)
}

/// The ▶ inside the disc of radius `r`, offsets from its centre.
fn play(dx: f32, dy: f32, r: f32) -> bool {
    let (left, right, half) = (-0.3 * r, 0.45 * r, 0.4 * r);
    dx >= left && dx <= right && dy.abs() <= half * (right - dx) / (right - left)
}

#[cfg(test)]
mod tests {
    #[test]
    fn play_sign_sits_inside_the_disc() {
        let r = super::SIDE as f32 / 2.0;
        assert!(super::play(0.0, 0.0, r));
        assert!(!super::play(-0.5 * r, 0.0, r));
        assert!(!super::play(0.0, 0.45 * r, r));
        assert!(super::picture(true).is_ok());
        assert!(super::picture(false).is_ok());
    }
}
