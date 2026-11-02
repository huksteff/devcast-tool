//! Leaving the project of the editor (7.28, mockup 6): closing the window, a new project, another
//! one, the copy of «Сохранить как…», another name. With unsaved changes closing and switching
//! ask «Сохранить / Не сохранять / Отмена» first, «Сохранить» by default (v1).

use std::path::PathBuf;

use dc_ui_kit::strings::projects as text;
use dc_ui_kit::widgets::{Choice, Chosen, panel_frame};
use eframe::egui::{self, Ui};

use super::EditorWindow;

/// Width of the question, points.
const WIDTH: f32 = 340.0;

/// What the editor asks the app to do with its project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leave {
    /// Close the window; the HUD comes back.
    Close,
    /// A new empty project in the recordings folder, in this window.
    New,
    /// The project in this folder, in this window.
    Open(PathBuf),
    /// The copy «Сохранить как…» made is ready here: it opens, this project drops its working
    /// state (it keeps what was saved).
    Copied(PathBuf),
    /// The project folder gets this name; the editor opens it again with its working state.
    Rename(String),
}

impl Leave {
    /// The working state is lost or replaced: unsaved changes are asked about first.
    fn asks(&self) -> bool {
        matches!(self, Self::Close | Self::New | Self::Open(_))
    }
}

/// The question while it is asked: what happens after it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Closing {
    #[default]
    Open,
    Asking(Leave),
}

impl EditorWindow {
    /// Leaves the project, asking first when there are unsaved changes and `leave` loses them.
    pub fn leave(&mut self, leave: Leave) {
        if leave.asks() && self.doc.unsaved() {
            self.closing = Closing::Asking(leave);
        } else {
            self.leaving = Some(leave);
        }
    }

    /// The question is open: shortcuts wait.
    pub fn asking(&self) -> bool {
        self.closing != Closing::Open
    }
}

/// Shows the question while it is asked and does the answer; an error to show (the project could
/// not be saved: the window stays).
pub fn show(ui: &Ui, editor: &mut EditorWindow) -> Option<String> {
    let Closing::Asking(leave) = &editor.closing else {
        return None;
    };
    let leave = leave.clone();
    let title = text::save_title(&editor.name);
    let mut answer = None;
    egui::Modal::new(egui::Id::unique("editor-save-question"))
        .frame(panel_frame())
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            answer = Choice {
                title: &title,
                text: text::UNSAVED_TEXT,
                cancel: text::CANCEL,
                other: text::DONT_SAVE,
                confirm: text::SAVE,
            }
            .show(ui);
        });
    let answer = answer?;
    editor.closing = Closing::Open;
    match answer {
        Chosen::Cancel => None,
        Chosen::Confirm => match editor.doc.save() {
            Ok(()) => {
                editor.leaving = Some(leave);
                None
            }
            Err(error) => Some(error.to_string()),
        },
        Chosen::Other => {
            editor.leaving = Some(leave);
            // The working state that stays is offered back when the project opens again.
            editor.doc.discard().err().map(|e| e.to_string())
        }
    }
}
