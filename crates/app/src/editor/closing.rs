//! Closing the editor when its edits cannot be written (decision of the owner 2026-11-02): the
//! first close stays open with the error, the next one asks whether to close without them.

use dc_ui_kit::strings::editor as text;
use dc_ui_kit::widgets::{Answer, Confirm, Kind, panel_frame};
use eframe::egui::{self, Ui};

/// Width of the question, points.
const WIDTH: f32 = 320.0;

/// Where closing the window stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Closing {
    #[default]
    Open,
    /// A close could not write the edits; the window stayed open with the error.
    Failed,
    /// «Закрыть без сохранения последних правок?» is shown.
    Asking,
}

impl Closing {
    /// After a close whose write failed: the first time the error is shown (`true`), then the
    /// question.
    pub fn failed(&mut self) -> bool {
        let first = *self == Self::Open;
        *self = if first { Self::Failed } else { Self::Asking };
        first
    }

    /// Shows the question while it is asked; `true` when the answer is to close without the
    /// edits.
    pub fn ask(&mut self, ui: &Ui) -> bool {
        if *self != Self::Asking {
            return false;
        }
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("editor-close-unsaved"))
            .frame(panel_frame())
            .show(ui.ctx(), |ui| {
                ui.set_width(WIDTH);
                answer = Confirm {
                    title: text::CLOSE_UNSAVED_TITLE,
                    text: text::CLOSE_UNSAVED_TEXT,
                    cancel: text::CLOSE_UNSAVED_CANCEL,
                    confirm: text::CLOSE_UNSAVED_CONFIRM,
                    kind: Kind::Danger,
                }
                .show(ui);
            });
        match answer {
            Some(Answer::Confirm) => true,
            Some(Answer::Cancel) => {
                *self = Self::Failed;
                false
            }
            None => false,
        }
    }
}
