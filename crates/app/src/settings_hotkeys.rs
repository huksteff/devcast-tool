//! «Горячие клавиши» of the settings window (mockup 5b): search by command or shortcut, the
//! global commands with their shortcut, capture «Нажмите сочетание…» (Esc cancels, Delete
//! clears, Ctrl or Alt needed), a conflict inside DevCast as a line under the command with
//! «Заменить» (swaps the shortcuts) / «Отмена», a shortcut the system holds as a yellow line
//! with «Изменить».

use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::hotkeys as text;
use dc_ui_kit::tokens::{
    BORDER, CONTROL, CONTROL_S, FS_S, ICON, PAUSE, PAUSE_SOFT, R1, S1, S2, S3, SURFACE_2,
    SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{Button, IconButton, Kind, search_field};
use eframe::egui::{self, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::hotkeys::{Command, combo, taken_by};
use crate::settings::Hotkeys;
use crate::settings_rows::{row, row_height, separator};

/// A shortcut another command already has, waiting for «Заменить» or «Отмена».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub command: Command,
    pub keys: String,
    pub owner: Command,
}

/// What the page keeps while the window is open.
#[derive(Debug, Default)]
pub struct HotkeyPage {
    pub search: String,
    /// The command whose shortcut is being pressed.
    pub capture: Option<Command>,
    pub conflict: Option<Conflict>,
}

/// Width of the shortcut button, points.
const KEYS: f32 = 200.0;

impl HotkeyPage {
    /// The page; `taken` are the commands whose shortcut the system did not give us.
    pub fn ui(&mut self, ui: &mut Ui, hotkeys: &mut Hotkeys, taken: &[Command]) {
        search_field(ui, &mut self.search, text::SEARCH, ui.available_width());
        ui.add_space(S3);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(text::GLOBAL)
                    .font(fonts::text(13.0, Weight::Semibold))
                    .color(TEXT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let reset = Button::new(text::RESET_ALL).kind(Kind::Ghost).small();
                if ui.add(reset.icon(Icon::Reset)).clicked() {
                    *hotkeys = Hotkeys::default();
                    self.capture = None;
                    self.conflict = None;
                }
            });
        });
        ui.add_space(S2);
        let search = self.search.to_lowercase();
        for command in Command::ALL {
            let keys = command.keys(hotkeys).clone().unwrap_or_default();
            let found = command.title().to_lowercase().contains(&search)
                || keys.to_lowercase().contains(&search);
            if found {
                self.row(ui, command, hotkeys, taken.contains(&command));
            }
        }
        ui.add_space(S3);
        ui.label(RichText::new(text::HELP).size(FS_S).color(TEXT_3));
        if let Some(command) = self.capture {
            self.listen(ui, command, hotkeys);
        }
    }

    fn row(&mut self, ui: &mut Ui, command: Command, hotkeys: &mut Hotkeys, taken: bool) {
        let capturing = self.capture == Some(command);
        let conflict = self.conflict.clone().filter(|c| c.command == command);
        row(ui, row_height(0.0), |ui| {
            let spacing = ui.spacing().item_spacing.x;
            let title = ui.available_width() - KEYS - CONTROL - 2.0 * spacing;
            let (rect, _) = ui.allocate_exact_size(vec2(title, 20.0), Sense::hover());
            ui.painter().text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                command.title(),
                fonts::text(13.0, Weight::Regular),
                TEXT,
            );
            let keys = command.keys(hotkeys).clone();
            let warn = conflict.is_some() || taken;
            if keys_button(ui, command, keys.as_deref(), capturing, warn).clicked() {
                self.capture = Some(command);
                self.conflict = None;
            }
            if capturing {
                if ui
                    .add(IconButton::new(Icon::Close, text::CANCEL_CAPTURE))
                    .clicked()
                {
                    self.capture = None;
                }
            } else if ui.add(IconButton::new(Icon::Reset, text::RESET)).clicked() {
                *command.keys_mut(hotkeys) = command.keys(&Hotkeys::default()).clone();
                self.conflict = None;
            }
        });
        if let Some(conflict) = conflict {
            let message = text::taken_by(&conflict.keys, conflict.owner.title());
            let answer = notice(ui, &message, &[text::REPLACE, text::CANCEL_CAPTURE]);
            if answer == Some(0) {
                let old = command.keys(hotkeys).clone();
                *conflict.owner.keys_mut(hotkeys) = old;
                *command.keys_mut(hotkeys) = Some(conflict.keys);
            }
            if answer.is_some() {
                self.conflict = None;
            }
        } else if taken
            && !capturing
            && notice(ui, text::TAKEN_BY_SYSTEM, &[text::CHANGE]).is_some()
        {
            self.capture = Some(command);
        }
        separator(ui);
    }

    /// Takes the shortcut pressed for `command`.
    fn listen(&mut self, ui: &Ui, command: Command, hotkeys: &mut Hotkeys) {
        let pressed = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((physical_key.unwrap_or(*key), *modifiers)),
                _ => None,
            })
        });
        let Some((key, modifiers)) = pressed else {
            return;
        };
        match key {
            egui::Key::Escape => self.capture = None,
            egui::Key::Delete => {
                *command.keys_mut(hotkeys) = None;
                self.capture = None;
            }
            _ => {
                let Some(keys) = combo(modifiers, key) else {
                    return;
                };
                self.capture = None;
                match taken_by(hotkeys, &keys, command) {
                    Some(owner) => {
                        self.conflict = Some(Conflict {
                            command,
                            keys,
                            owner,
                        });
                    }
                    None => *command.keys_mut(hotkeys) = Some(keys),
                }
            }
        }
    }
}

/// The shortcut as key caps, «Нажмите сочетание…» while capturing, «Нет» when cleared; a
/// yellow frame on a conflict.
fn keys_button(
    ui: &mut Ui,
    command: Command,
    keys: Option<&str>,
    capturing: bool,
    warn: bool,
) -> egui::Response {
    let width = KEYS;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let label = format!("{}: {}", command.title(), keys.unwrap_or(text::NONE));
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &label));
    let (fill, border) = match (capturing, warn) {
        (true, _) => (SURFACE_2, dc_ui_kit::tokens::ACCENT_HI),
        (false, true) => (PAUSE_SOFT, PAUSE),
        (false, false) if response.hovered() => (SURFACE_3, BORDER),
        (false, false) => (SURFACE_2, BORDER),
    };
    let painter = ui.painter();
    painter.rect(rect, R1, fill, Stroke::new(1.0, border), StrokeKind::Inside);
    let left = rect.left_center() + vec2(S2, 0.0);
    match keys.filter(|_| !capturing) {
        Some(keys) => {
            let mut x = left.x;
            for (i, part) in keys.split('+').enumerate() {
                if i > 0 {
                    let plus = painter.text(
                        egui::pos2(x + S1, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "+",
                        fonts::text(FS_S, Weight::Regular),
                        TEXT_3,
                    );
                    x = plus.right() + S1;
                }
                let galley = painter.layout_no_wrap(part.to_owned(), fonts::mono(FS_S), TEXT);
                let cap = egui::Rect::from_min_size(
                    egui::pos2(x, rect.center().y - 10.0),
                    vec2(galley.size().x + 2.0 * 6.0, 20.0),
                );
                painter.rect(
                    cap,
                    4,
                    SURFACE_3,
                    Stroke::new(1.0, BORDER),
                    StrokeKind::Inside,
                );
                painter.galley(
                    egui::pos2(cap.left() + 6.0, cap.center().y - galley.size().y / 2.0),
                    galley,
                    TEXT,
                );
                x = cap.right();
            }
        }
        None => {
            let (caption, color) = if capturing {
                (text::PRESS, TEXT_2)
            } else {
                (text::NONE, TEXT_3)
            };
            painter.text(
                left,
                egui::Align2::LEFT_CENTER,
                caption,
                fonts::text(FS_S, Weight::Regular),
                color,
            );
        }
    }
    response
}

/// A yellow line under a command: ⚠, the message and buttons; returns the clicked button.
fn notice(ui: &mut Ui, message: &str, buttons: &[&str]) -> Option<usize> {
    let mut clicked = None;
    egui::Frame::new()
        .fill(PAUSE_SOFT)
        .corner_radius(R1)
        .inner_margin(egui::Margin::symmetric(S2 as i8, S1 as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
                Icon::Warn.paint(ui.painter(), rect, PAUSE);
                let spacing = ui.spacing().item_spacing.x;
                let taken: f32 = buttons
                    .iter()
                    .map(|caption| Button::new(caption).small().size(ui).x + spacing)
                    .sum();
                let room = ui.available_width() - taken;
                ui.allocate_ui(vec2(room, CONTROL_S), |ui| {
                    ui.set_width(room);
                    ui.add(egui::Label::new(RichText::new(message).size(FS_S).color(TEXT)).wrap());
                });
                for (i, caption) in buttons.iter().enumerate() {
                    if ui.add(Button::new(caption).small()).clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    clicked
}

#[cfg(test)]
#[path = "settings_hotkeys_tests.rs"]
mod tests;
