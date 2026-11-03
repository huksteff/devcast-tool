//! «Горячие клавиши» of the settings window (mockup 5b): search by command or shortcut, the
//! global hotkeys and the editor actions (`keymap.rs`) with their shortcut, capture «Нажмите
//! сочетание…» (Esc cancels, Backspace clears, a global one needs Ctrl or Alt), a conflict with
//! any other shortcut as a line under the command with «Заменить» (swaps the shortcuts) /
//! «Отмена», a global shortcut the system holds as a yellow line with «Изменить».

use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::{hotkeys as text, keys as names};
use dc_ui_kit::tokens::{
    BORDER, CONTROL, CONTROL_S, FS_S, ICON, PAUSE, PAUSE_SOFT, R1, S1, S2, S3, S5, SURFACE_2,
    SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::{Button, IconButton, Kind, search_field};
use eframe::egui::{self, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::hotkeys::Command;
use crate::keymap::{Binding, EditorKey, Title, taken_by};
use crate::settings::AppSettings;
use crate::shortcut::Caps;
use dc_settings::Shortcut;
use dc_ui_kit::settings_rows::{row, row_height, separator};

/// A shortcut another command already has, waiting for «Заменить» or «Отмена».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub binding: Binding,
    pub keys: String,
    pub owner: Binding,
}

/// What the page keeps while the window is open.
#[derive(Debug, Default)]
pub struct HotkeyPage {
    pub search: String,
    /// The command whose shortcut is being pressed.
    pub capture: Option<Binding>,
    pub conflict: Option<Conflict>,
}

/// Width of the shortcut button, points.
const KEYS: f32 = 200.0;

impl HotkeyPage {
    /// The page; `taken` are the global hotkeys the system did not give us.
    pub fn ui(&mut self, ui: &mut Ui, settings: &mut AppSettings, taken: &[Command]) {
        search_field(ui, &mut self.search, text::SEARCH, ui.available_width());
        ui.add_space(S3);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let global = Command::ALL.map(Binding::Global);
                self.group(ui, text::GLOBAL, &global, settings, taken);
                ui.add_space(S5);
                let editor = EditorKey::ALL.map(Binding::Editor);
                self.group(ui, names::EDITOR_GROUP, &editor, settings, taken);
                ui.add_space(S3);
                ui.label(RichText::new(text::HELP).size(FS_S).color(TEXT_3));
            });
        if let Some(binding) = self.capture {
            self.listen(ui, binding, settings);
        }
    }

    /// A titled group with «Сбросить все» of its bindings and the rows the search finds; nothing
    /// when it finds none.
    fn group(
        &mut self,
        ui: &mut Ui,
        title: &str,
        bindings: &[Binding],
        settings: &mut AppSettings,
        taken: &[Command],
    ) {
        let search = self.search.to_lowercase();
        let found: Vec<Binding> = bindings
            .iter()
            .copied()
            .filter(|binding| {
                let keys = binding.keys(settings).unwrap_or_default();
                let caps = Shortcut::parse(&keys)
                    .map(Shortcut::label)
                    .unwrap_or_default();
                [binding.title(), &keys, &caps]
                    .iter()
                    .any(|t| t.to_lowercase().contains(&search))
            })
            .collect();
        if found.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .font(fonts::text(13.0, Weight::Semibold))
                    .color(TEXT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let reset = Button::new(text::RESET_ALL).kind(Kind::Ghost).small();
                if ui.add(reset.icon(Icon::Reset)).clicked() {
                    for binding in bindings {
                        binding.reset(settings);
                    }
                    self.capture = None;
                    self.conflict = None;
                }
            });
        });
        ui.add_space(S2);
        for binding in found {
            let system = matches!(binding, Binding::Global(c) if taken.contains(&c));
            self.row(ui, binding, settings, system);
        }
    }

    fn row(&mut self, ui: &mut Ui, binding: Binding, settings: &mut AppSettings, taken: bool) {
        let capturing = self.capture == Some(binding);
        let conflict = self.conflict.clone().filter(|c| c.binding == binding);
        row(ui, row_height(0.0), |ui| {
            let spacing = ui.spacing().item_spacing.x;
            let title = ui.available_width() - KEYS - CONTROL - 2.0 * spacing;
            let (rect, _) = ui.allocate_exact_size(vec2(title, 20.0), Sense::hover());
            ui.painter().text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                binding.title(),
                fonts::text(13.0, Weight::Regular),
                TEXT,
            );
            let keys = binding.keys(settings);
            let warn = conflict.is_some() || taken;
            if keys_button(ui, binding, keys.as_deref(), capturing, warn).clicked() {
                self.capture = Some(binding);
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
                binding.reset(settings);
                self.conflict = None;
            }
        });
        if let Some(conflict) = conflict {
            let message = text::taken_by(&conflict.keys, conflict.owner.title());
            let answer = notice(ui, &message, &[text::REPLACE, text::CANCEL_CAPTURE]);
            if answer == Some(0) {
                let old = binding.keys(settings);
                conflict.owner.set(settings, old);
                binding.set(settings, Some(conflict.keys));
            }
            if answer.is_some() {
                self.conflict = None;
            }
        } else if taken
            && !capturing
            && notice(ui, text::TAKEN_BY_SYSTEM, &[text::CHANGE]).is_some()
        {
            self.capture = Some(binding);
        }
        separator(ui);
    }

    /// Takes the shortcut pressed for `binding`.
    fn listen(&mut self, ui: &Ui, binding: Binding, settings: &mut AppSettings) {
        let pressed = ui.input(|i| {
            i.events
                .iter()
                .find_map(|event| Shortcut::of_event(event, i.modifiers))
        });
        let Some(pressed) = pressed else {
            return;
        };
        match pressed.key {
            egui::Key::Escape => self.capture = None,
            egui::Key::Backspace => {
                binding.set(settings, None);
                self.capture = None;
            }
            _ if !binding.takes(pressed) => {}
            _ => {
                self.capture = None;
                let keys = pressed.text();
                match taken_by(settings, pressed, binding) {
                    Some(owner) => {
                        self.conflict = Some(Conflict {
                            binding,
                            keys,
                            owner,
                        });
                    }
                    None => binding.set(settings, Some(keys)),
                }
            }
        }
    }
}

/// The shortcut as key caps, «Нажмите сочетание…» while capturing, «Нет» when cleared; a
/// yellow frame on a conflict. The accessible name is «command: stored text».
fn keys_button(
    ui: &mut Ui,
    binding: Binding,
    keys: Option<&str>,
    capturing: bool,
    warn: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(KEYS, 30.0), Sense::click());
    let label = format!("{}: {}", binding.title(), keys.unwrap_or(text::NONE));
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
    let caps = keys
        .filter(|_| !capturing)
        .map(|keys| Shortcut::parse(keys).map_or_else(|| vec![keys], |s| s.caps()));
    match caps {
        Some(caps) => {
            crate::keycaps::paint(painter, left, &caps);
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
