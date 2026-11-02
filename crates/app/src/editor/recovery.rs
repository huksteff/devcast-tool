//! «Найдена несохранённая работа» (7.28, mockup 6): a project opens with the working state a
//! crash, leaving the app or a failed «Не сохранять» left on disk. «Восстановить» goes on with
//! it, «Отбросить» goes back to `project.json` (`Document::discard`).

use std::time::Instant;

use dc_ui_kit::strings::projects as text;
use dc_ui_kit::widgets::{Answer, Confirm, Kind, panel_frame};
use eframe::egui::{self, Ui};

use super::EditorWindow;

/// Width of the question, points.
const WIDTH: f32 = 340.0;

/// Shows the question while it is asked and does the answer at `now`; why discarding failed.
pub fn show(ui: &Ui, editor: &mut EditorWindow, now: Instant) -> Option<String> {
    if !editor.recovery {
        return None;
    }
    let body = text::recovery_text(&editor.name);
    let mut answer = None;
    egui::Modal::new(egui::Id::unique("editor-recovery"))
        .frame(panel_frame())
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            answer = Confirm {
                title: text::RECOVERY_TITLE,
                text: &body,
                cancel: text::DISCARD,
                confirm: text::RESTORE,
                kind: Kind::Primary,
            }
            .show(ui);
        });
    editor.recovery = answer.is_none();
    match answer? {
        Answer::Confirm => None,
        Answer::Cancel => match editor.doc.discard() {
            Ok(()) => editor.refresh(now),
            Err(error) => Some(error.to_string()),
        },
    }
}
