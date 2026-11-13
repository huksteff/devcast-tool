//! Editing on the preview (7.21–7.24): the gestures of the effects over the output frame — the
//! frame of the picked zoom, the brush in the picked drawing, the boxes of the annotations — are
//! the table `lanes::STAGES` (`dc_ui_kit::stage::Stage`); the editor only hosts them: it offers
//! the pointer to each in turn until one takes it, keeps the gesture going on (`View::stage`) and
//! the stages' window choices (`View::kept`), and does what they answer.
//!
//! A gesture is one registry command on release (`Action::Apply`, one undo step, the same as
//! `devcast`). While it goes on, the preview renders the draft: the project as that command would
//! make it (`Document::draft`), so the result shows live; the release, a refused command or Esc
//! bring the preview back to the project.

#[cfg(test)]
mod brush_tests;
#[cfg(test)]
mod tests;

use dc_ops::Step;
use dc_render::Camera;
use dc_ui_kit::stage::geom::Mapping;
use dc_ui_kit::stage::{Answer, Cx, Pick, Picked, Shown};
use dc_ui_kit::strings::editor as text;
use eframe::egui::{self, Rect, Response, Sense, Ui, WidgetInfo};

use super::lanes::STAGES;
use super::lanes::annotations::AnnotId;
use super::selection::{Item, Selected};
use super::{Action, EditorWindow};

/// The object picked alone, as the stages see it.
fn picked_alone(editor: &EditorWindow) -> Option<Picked> {
    match editor.view.selection.single()? {
        Selected::Region(item) => Some(Picked::Region(item)),
        Selected::Key { id, .. } => Some(Picked::Key(Item::of(id))),
        _ => None,
    }
}

/// The annotation picked alone (as a region or by one of its keys).
pub fn picked(editor: &EditorWindow) -> Option<AnnotId> {
    picked_alone(editor)?.item().id()
}

/// The preview shows the picture with the camera: no stage asks for it bare.
pub fn camera(editor: &EditorWindow) -> bool {
    let picked = picked_alone(editor);
    !STAGES.iter().any(|stage| stage.bare(picked))
}

/// Gives `with` what the editor shows the stages this frame.
fn with_cx<R>(editor: &mut EditorWindow, with: impl FnOnce(&mut Cx<'_>) -> R) -> R {
    let picked = picked_alone(editor);
    let rows = editor.tracks.rows.iter().filter(|t| !t.off);
    let shown: Vec<_> = rows
        .flat_map(|t| {
            t.regions.iter().filter_map(|r| {
                Some(Shown {
                    track: t.row.track,
                    item: r.item?,
                    range: r.range,
                    keys: &r.keys,
                })
            })
        })
        .collect();
    let view = &mut editor.view;
    let mut cx = Cx {
        project: editor.doc.project(),
        picked,
        shown: &shown,
        playhead: view.playhead,
        auto_keys: view.auto_keys,
        magnet: view.magnet,
        held: &mut view.stage,
        kept: &mut view.kept,
    };
    with(&mut cx)
}

/// The pointer over the output frame `frame` (screen) of the preview; `response` is the preview
/// area's, `camera` that of the frame shown (none before the first frame: at rest).
pub fn ui(
    ui: &Ui,
    editor: &mut EditorWindow,
    response: &Response,
    frame: Rect,
    camera: Option<Camera>,
) {
    let Some(canvas) = editor.canvas else {
        return;
    };
    ui.interact(frame, ui.scope_id().with("preview-frame"), Sense::hover())
        .widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, text::PREVIEW_FRAME));
    for stage in STAGES {
        let bare = stage.bare(picked_alone(editor));
        // A bare stage works over the frame asked without the camera.
        let camera = match bare {
            true => Camera::IDENTITY,
            false => camera.unwrap_or(Camera::IDENTITY),
        };
        let map = Mapping::new(frame, canvas, camera);
        let answer = with_cx(editor, |cx| stage.ui(ui, cx, response, map));
        let ours = answer.ours;
        settle(ui, editor, answer);
        if ours {
            return;
        }
    }
}

/// The stages' bars over the top of the preview `area`, after the frame.
pub fn bars(ui: &Ui, editor: &mut EditorWindow, area: Rect) {
    for stage in STAGES {
        with_cx(editor, |cx| stage.bar(ui, cx, area));
    }
}

/// What a stage answered: the pick, the draft shown, the end applying its commands.
fn settle(ui: &Ui, editor: &mut EditorWindow, answer: Answer) {
    match answer.pick {
        Some(Pick::Only(item)) => editor.view.selection.only(Selected::Region(item)),
        Some(Pick::Nothing) => editor.view.selection.clear(),
        None => {}
    }
    if let Some(steps) = answer.done {
        let drafted = editor.view.stage.take().is_some_and(|held| held.drafted());
        finish(editor, drafted, steps);
        return;
    }
    if let Some(steps) = answer.draft {
        show_draft(ui, editor, &steps);
    }
}

/// Shows the project as `steps` make it on the preview (a gesture on it, a held control of the
/// inspector). A draft the commands refuse is not shown: the release applies the same steps
/// and its toast says why.
pub(super) fn show_draft(ui: &Ui, editor: &mut EditorWindow, steps: &[Step]) {
    if let Ok(draft) = editor.doc.draft(steps) {
        editor.preview.changed(&draft);
        // The frame of the draft is asked by the next pass.
        ui.ctx().request_repaint();
    }
}

/// The gesture ended: `steps` (none when nothing changed) are applied as one undo step, the
/// preview goes back to the project when it showed a draft.
fn finish(editor: &mut EditorWindow, drafted: bool, steps: Option<Vec<Step>>) {
    if drafted {
        editor.preview.changed(editor.doc.project());
    }
    if let Some(steps) = steps {
        editor.ask(Action::Apply(steps));
    }
}

/// Esc during a gesture: it ends without a change. True when there was one.
pub fn cancel(editor: &mut EditorWindow) -> bool {
    let Some(held) = editor.view.stage.take() else {
        return false;
    };
    if held.drafted() {
        editor.preview.changed(editor.doc.project());
    }
    true
}
