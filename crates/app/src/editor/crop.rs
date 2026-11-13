//! The «Кроп и формат» dialog in the editor: the dialog itself is `dc_fx_crop::dialog`; the
//! editor opens it for the picked clip, otherwise the one under the playhead
//! (`Action::OpenCrop`, playback pauses), gives it the clip's source frame at the playhead (at
//! its start when the playhead is elsewhere) from the preview's render path (`Preview::source`)
//! and runs «Применить» as one undo step.

use dc_core::OutputTime;
use dc_fx_crop::dialog::{self, Answer, Dialog};
use eframe::egui::Ui;

use super::timeline::Row;
use super::{Action, EditorWindow};

/// The dialog while it is open and the moment whose picture it shows (inside the clip).
#[derive(Debug)]
pub struct CropDialog {
    pub dialog: Dialog,
    time: OutputTime,
}

impl CropDialog {
    /// The dialog of the picked clip, or of the one under the playhead; `None` without clips.
    pub fn open(editor: &EditorWindow) -> Option<Self> {
        let regions = editor
            .tracks
            .rows
            .iter()
            .find(|t| t.row == Row::CLIPS)
            .map(|t| t.regions.as_slice())
            .unwrap_or_default();
        let picked: Vec<_> = editor.view.selection.clips().collect();
        let at = editor.view.playhead;
        let region = match picked.as_slice() {
            [one] => regions.iter().find(|r| r.clip == Some(*one)),
            _ => None,
        }
        .or_else(|| regions.iter().find(|r| r.range.contains(at)))
        .or_else(|| regions.first())?;
        let time = match region.range.contains(at) {
            true => at,
            false => region.range.start(),
        };
        let dialog = Dialog::open(editor.doc.project(), region.clip?, region.label.clone())?;
        Some(Self { dialog, time })
    }
}

/// Shows the open dialog; «Применить» queues its commands, the dialog closes on an answer.
pub fn show(ui: &Ui, editor: &mut EditorWindow) {
    let Some(open) = editor.crop.as_mut() else {
        return;
    };
    let time = open.time;
    let preview = &mut editor.preview;
    let answer = dialog::show(ui, &mut open.dialog, &mut |size| {
        let shown = preview.source(time, size);
        shown.texture.ok_or_else(|| shown.error.unwrap_or_default())
    });
    match answer {
        Some(Answer::Apply(steps)) => {
            if !steps.is_empty() {
                editor.ask(Action::Apply(steps));
            }
            editor.crop = None;
        }
        Some(Answer::Cancel) => editor.crop = None,
        None => {}
    }
}
