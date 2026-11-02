//! Editing on the preview (7.21–7.24): the frame of the picked zoom (`zoom.rs`) and the boxes of
//! the annotations (`annot.rs`) follow the pointer over the output frame, with guides and
//! snapping (`snap.rs`); the brush draws in the picked drawing (`brush.rs`); the math is
//! `geom.rs`, the drawing `paint.rs`.
//!
//! A gesture is one registry command on release (`Action::Apply`, one undo step, the same as
//! `devcast`). While it goes on, the preview renders the draft: the project as that command would
//! make it (`Document::draft`), so the result shows live; the release, a refused command or Esc
//! bring the preview back to the project.

mod annot;
mod brush;
mod geom;
mod paint;
mod snap;
mod zoom;

#[cfg(test)]
mod brush_tests;
#[cfg(test)]
mod geom_tests;
#[cfg(test)]
mod tests;

pub use annot::picked;
pub use brush::bar as brush_bar;

use dc_fx_zoom::ZoomId;
use dc_ops::Step;
use dc_render::Camera;
use dc_ui_kit::strings::editor as text;
use eframe::egui::{self, Rect, Response, Sense, Ui, WidgetInfo};

use super::selection::{Item, Selected};
use super::{Action, EditorWindow};
use geom::Mapping;

/// A gesture on the preview.
#[derive(Debug, Clone, PartialEq)]
pub enum Drag {
    Zoom(zoom::ZoomDrag),
    Annot(annot::AnnotDrag),
    Brush(brush::BrushDrag),
}

impl Drag {
    /// The preview shows a draft of it.
    fn drafted(&self) -> bool {
        match self {
            Self::Zoom(drag) => drag.drafted,
            Self::Annot(drag) => drag.drafted,
            Self::Brush(drag) => drag.drafted,
        }
    }
}

/// The zoom picked alone: the preview shows its frame over the picture without the camera.
pub fn picked_zoom(editor: &EditorWindow) -> Option<ZoomId> {
    match editor.view.selection.single()? {
        Selected::Region(Item::Zoom(id)) => Some(id),
        _ => None,
    }
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
    if let Some(id) = picked_zoom(editor) {
        // The frame is asked without the camera while a zoom is picked.
        let map = Mapping::new(frame, canvas, Camera::IDENTITY);
        if zoom::ui(ui, editor, response, map, id) {
            return;
        }
    }
    let map = Mapping::new(frame, canvas, camera.unwrap_or(Camera::IDENTITY));
    annot::ui(ui, editor, response, map);
}

/// Shows the project as `steps` make it on the preview. A draft the commands refuse is not
/// shown: the release applies the same steps and its toast says why.
fn show_draft(ui: &Ui, editor: &mut EditorWindow, steps: &[Step]) {
    if let Ok(draft) = editor.doc.draft(steps) {
        editor.preview.changed(&draft);
        // The frame of the draft is asked by the next pass.
        ui.ctx().request_repaint();
    }
}

/// The gesture ends: `steps` (none when nothing changed) are applied as one undo step, the
/// preview goes back to the project when it showed a draft.
fn finish(editor: &mut EditorWindow, drafted: bool, steps: Option<Vec<Step>>) {
    editor.view.stage = None;
    if drafted {
        editor.preview.changed(editor.doc.project());
    }
    if let Some(steps) = steps {
        editor.ask(Action::Apply(steps));
    }
}

/// Esc during a gesture: it ends without a change. True when there was one.
pub fn cancel(editor: &mut EditorWindow) -> bool {
    let Some(drag) = editor.view.stage.take() else {
        return false;
    };
    if drag.drafted() {
        editor.preview.changed(editor.doc.project());
    }
    true
}
