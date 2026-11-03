//! The brush on the preview (7.24): the stroke itself is the drawing's stage (`lanes::brush`:
//! tool, colour and thickness, points, `drawing add`, the bar); here it is hosted — the stroke
//! starts over the picked drawing, follows the pointer (Alt: a line from the first point) and
//! ends as one undo step; the tool and the eraser's size of the bar are the window's
//! (`View::brush`, `View::eraser`), not the project's.

use eframe::egui::{Pos2, Rect, Ui};

use super::annot::picked;
use super::{Drag, finish, show_draft};
use crate::editor::EditorWindow;
use crate::editor::lanes::boxes::OnFrame;
use crate::editor::lanes::brush::{self, Brush, Brushing};
use dc_ui_kit::stage::geom::Mapping;

/// What the bar chose.
fn chosen(editor: &EditorWindow) -> Brush {
    Brush {
        tool: editor.view.brush,
        eraser: editor.view.eraser,
    }
}

/// Tool, colour and thickness of a stroke of drawing `on` begun now, Shift held or not.
fn pick(editor: &EditorWindow, on: &OnFrame, shift: bool) -> Option<brush::Picked> {
    let project = editor.doc.project();
    brush::pick(project, (on, editor.view.playhead), chosen(editor), shift)
}

/// The pointer over the picked drawing `on`: the brush circle, the eraser's with Shift.
pub fn hover(ui: &Ui, editor: &EditorWindow, on: &OnFrame, map: Mapping, at: Pos2) {
    let held = match &editor.view.stage {
        Some(Drag::Brush(drag)) => Some(drag.width),
        _ => None,
    };
    let shift = ui.input(|i| i.modifiers.shift);
    if let Some(width) = held.or_else(|| pick(editor, on, shift).map(|(_, _, w)| w)) {
        brush::hover(ui, map, at, width);
    }
}

/// A left drag begins at `press` (screen) inside the picked drawing `on`.
pub fn start(ui: &Ui, editor: &mut EditorWindow, on: &OnFrame, map: Mapping, press: Pos2) {
    let shift = ui.input(|i| i.modifiers.shift);
    let Some(picked) = pick(editor, on, shift) else {
        return;
    };
    let stroke = Brushing::new(on, picked, map.canvas(press));
    editor.view.stage = Some(Drag::Brush(stroke));
}

/// The pointer at `p` (canvas px) during the stroke.
pub fn drag(ui: &Ui, editor: &mut EditorWindow, p: Pos2) {
    let line = ui.input(|i| i.modifiers.alt);
    let Some(Drag::Brush(drag)) = &mut editor.view.stage else {
        return;
    };
    if drag.add(p, line) {
        let steps = vec![drag.step()];
        show_draft(ui, editor, &steps);
    }
}

/// The release: the stroke as one undo step.
pub fn stop(editor: &mut EditorWindow) {
    if let Some(Drag::Brush(drag)) = editor.view.stage.clone() {
        finish(editor, drag.drafted, Some(vec![drag.step()]));
    }
}

/// The brush bar over the top of the preview `area` while a drawing is picked alone.
pub fn bar(ui: &Ui, editor: &mut EditorWindow, area: Rect) {
    let Some(id) = picked(editor) else {
        return;
    };
    if !brush::is_drawing(editor.doc.project(), id) {
        return;
    }
    let mut chosen = chosen(editor);
    brush::bar(ui, &mut chosen, area);
    editor.view.brush = chosen.tool;
    editor.view.eraser = chosen.eraser;
}
