//! Question and error panels (`.pop` of the mockup): a bold title, grey text, buttons on the
//! right. The caller puts them where they belong: a popover of the HUD (it is excluded from
//! capture, a system dialog would get into the video) or an `egui::Modal` in other windows.

use egui::{Frame, Key, Layout, Stroke, Ui};

use crate::fonts::{self, Weight};
use crate::strings::kit;
use crate::theme::SHADOW;
use crate::tokens::{BORDER, CONTROL_S, FS_M, FS_S, R3, S1, S2, S3, SURFACE, TEXT, TEXT_2};
use crate::widgets::{Button, Kind};

/// Frame of popovers and dialogs: surface, border, radius 12, shadow, 12 px inside.
pub fn panel_frame() -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(R3)
        .shadow(SHADOW)
        .inner_margin(S3)
}

/// What the user answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The safe button or Esc.
    Cancel,
    Confirm,
}

/// A yes/no question; Esc is «Cancel».
pub struct Confirm<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub cancel: &'a str,
    pub confirm: &'a str,
    /// Colour of the confirm button: `Danger` for deleting.
    pub kind: Kind,
}

impl Confirm<'_> {
    pub fn show(&self, ui: &mut Ui) -> Option<Answer> {
        heading(ui, self.title, self.text);
        let mut answer = None;
        buttons(ui, |ui| {
            ui.spacing_mut().item_spacing.x = S2;
            if ui
                .add(Button::new(self.confirm).kind(self.kind).small())
                .clicked()
            {
                answer = Some(Answer::Confirm);
            }
            if ui.add(Button::new(self.cancel).small()).clicked() {
                answer = Some(Answer::Cancel);
            }
        });
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            answer = Some(Answer::Cancel);
        }
        answer
    }
}

/// What the user did with an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorAction {
    Close,
    /// One of the extra buttons, by its index.
    Extra(usize),
}

/// An error with «Копировать» (puts title and text on the clipboard) and «Закрыть».
pub struct ErrorPanel<'a> {
    pub title: &'a str,
    pub text: &'a str,
    /// Buttons before «Закрыть», like «Открыть папку записи».
    pub extra: &'a [&'a str],
}

impl ErrorPanel<'_> {
    pub fn show(&self, ui: &mut Ui) -> Option<ErrorAction> {
        heading(ui, self.title, self.text);
        let mut action = None;
        buttons(ui, |ui| {
            ui.spacing_mut().item_spacing.x = S2;
            if ui
                .add(Button::new(kit::CLOSE).kind(Kind::Primary).small())
                .clicked()
            {
                action = Some(ErrorAction::Close);
            }
            for (i, extra) in self.extra.iter().enumerate().rev() {
                if ui.add(Button::new(extra).small()).clicked() {
                    action = Some(ErrorAction::Extra(i));
                }
            }
            if ui.add(Button::new(kit::COPY).small()).clicked() {
                ui.ctx().copy_text(format!("{}\n{}", self.title, self.text));
            }
        });
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            action = Some(ErrorAction::Close);
        }
        action
    }
}

fn heading(ui: &mut Ui, title: &str, text: &str) {
    ui.label(
        egui::RichText::new(title)
            .font(fonts::text(FS_M, Weight::Semibold))
            .color(TEXT),
    );
    ui.add_space(S1);
    ui.label(
        egui::RichText::new(text)
            .font(fonts::text(FS_S, Weight::Regular))
            .color(TEXT_2),
    );
    ui.add_space(S3);
}

/// The row of small buttons on the right, one button high: a right-to-left layout centred
/// vertically would take all the height left (a popover lays out in its full height).
fn buttons(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let size = egui::vec2(ui.available_width(), CONTROL_S);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(egui::Align::Center), add);
}
