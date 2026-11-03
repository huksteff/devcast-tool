//! Annotations on the preview (7.22): the boxes the annotations' stage gives
//! (`lanes::boxes::shown`, from the annotation rows shown), picking and which gesture runs. A
//! click picks the topmost annotation under the pointer, a click on nothing drops the selection.
//! The picked one's box drags, its handles resize it, its knob turns it (`boxes::Drag`); the edges
//! and the centre stick to the frame and to the other annotations shown (7.23) while the snapping
//! (N) is on and Ctrl is up. One `annot set` per gesture; the preview shows the draft meanwhile.
//! A left drag inside the picked drawing draws instead (`brush.rs`); its box moves with a right
//! drag (mockup 3c).

use dc_ui_kit::stage::geom::{GRAB, Grip, KNOB, Mapping, Resize};
use dc_ui_kit::stage::paint;
use dc_ui_kit::stage::snap::Lines;
use eframe::egui::{PointerButton, Pos2, Response, Ui, Vec2};

use super::{Drag, brush, finish, show_draft};
use crate::editor::EditorWindow;
use crate::editor::lanes::annotations::AnnotId;
use crate::editor::lanes::boxes::{self, OnFrame, Placed};
use crate::editor::selection::{Item, Selected};
use crate::editor::timeline::Row;

/// The annotations shown at the playhead, bottom to top, with their boxes on a `canvas` px
/// frame; hidden rows do not count.
pub fn shown(editor: &EditorWindow, canvas: Vec2) -> Vec<OnFrame> {
    let rows = editor.tracks.rows.iter().filter(|t| !t.off);
    let placed = rows.filter_map(|t| match t.row {
        Row::Annotations(track) => Some((track, &t.regions)),
        _ => None,
    });
    let regions = placed.flat_map(|(track, regions)| {
        regions.iter().filter_map(move |r| {
            Some(Placed {
                track,
                item: r.item?,
                range: r.range,
                keys: &r.keys,
            })
        })
    });
    let view = &editor.view;
    let project = editor.doc.project();
    boxes::shown(project, (view.playhead, view.auto_keys), regions, canvas)
}

/// The annotation picked alone (as a region or by one of its keys).
pub fn picked(editor: &EditorWindow) -> Option<AnnotId> {
    match editor.view.selection.single()? {
        Selected::Region(item) => item.id(),
        Selected::Key { id, .. } => Some(id),
        _ => None,
    }
}

/// Picking, the gestures on the picked box and its drawing.
pub fn ui(ui: &Ui, editor: &mut EditorWindow, response: &Response, map: Mapping) {
    let shown = shown(editor, map.canvas);
    let picked = picked(editor).and_then(|id| shown.iter().find(|s| s.edit.id == id));
    let (grab, knob) = (map.length(GRAB), map.length(KNOB));
    let held = match &editor.view.stage {
        Some(Drag::Annot(drag)) => Some(drag.grip),
        _ => None,
    };
    let pointer = response.hover_pos();
    let own = |p: Pos2| picked.and_then(|s| s.boxed.grip(map.canvas(p), grab, knob));
    let hovered =
        pointer.and_then(|p| own(p).or_else(|| topmost(&shown, map.canvas(p)).map(|_| Grip::Move)));
    let brushing = matches!(editor.view.stage, Some(Drag::Brush(_)));
    let drawing = picked.filter(|s| s.draws());
    let over_drawing =
        drawing.is_some() && held.is_none() && pointer.and_then(own) == Some(Grip::Move);
    match (drawing, pointer) {
        (Some(on), Some(at)) if brushing || over_drawing => brush::hover(ui, editor, on, map, at),
        _ => {
            if let Some(grip) = held.or(hovered) {
                let angle = picked.map_or(0.0, |s| s.boxed.angle);
                ui.ctx().set_cursor_icon(boxes::cursor(grip, angle));
            }
        }
    }
    gestures(ui, editor, response, &shown, picked, map);
    let id = picked.map(|s| s.edit.id);
    let (boxed, guides) = match &editor.view.stage {
        Some(Drag::Annot(drag)) if Some(drag.edit.id) == id => {
            (Some(drag.now), drag.guides.clone())
        }
        _ => (picked.map(|s| s.boxed), Vec::new()),
    };
    paint::guides(ui, map, &guides);
    if let Some(boxed) = boxed {
        paint::selected(ui, map, &boxed);
    }
}

/// The press, click, drag and release on the preview: a left drag inside the picked drawing
/// draws (`brush.rs`), anything else picks, moves, resizes or turns.
fn gestures(
    ui: &Ui,
    editor: &mut EditorWindow,
    response: &Response,
    shown: &[OnFrame],
    picked: Option<&OnFrame>,
    map: Mapping,
) {
    if response.drag_started()
        && let Some(press) = ui.input(|i| i.pointer.press_origin())
    {
        let (grab, knob) = (map.length(GRAB), map.length(KNOB));
        let drawing = picked.filter(|s| {
            s.draws()
                && response.drag_started_by(PointerButton::Primary)
                && s.boxed.grip(map.canvas(press), grab, knob) == Some(Grip::Move)
        });
        match drawing {
            Some(on) => brush::start(ui, editor, on, map, press),
            None => start(editor, shown, picked, map, press),
        }
    }
    if response.clicked()
        && let Some(at) = response.interact_pointer_pos()
    {
        pick(editor, shown, map.canvas(at));
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        match editor.view.stage {
            Some(Drag::Brush(_)) => brush::drag(ui, editor, map.canvas(pointer)),
            _ => drag(ui, editor, shown, map, map.canvas(pointer)),
        }
    }
    if response.drag_stopped() {
        match editor.view.stage.clone() {
            Some(Drag::Annot(drag)) => {
                let changed = drag.changed();
                finish(editor, drag.drafted, changed.then(|| vec![drag.step()]));
            }
            Some(Drag::Brush(_)) => brush::stop(editor),
            _ => {}
        }
    }
}

/// The topmost of `shown` under `p` (canvas px).
fn topmost(shown: &[OnFrame], p: Pos2) -> Option<&OnFrame> {
    shown.iter().rev().find(|s| s.boxed.contains(p))
}

/// A click: the topmost annotation under it gets picked; nothing under it drops the selection.
fn pick(editor: &mut EditorWindow, shown: &[OnFrame], p: Pos2) {
    let selection = &mut editor.view.selection;
    match topmost(shown, p) {
        Some(on) => selection.only(Selected::Region(Item::of(on.edit.id))),
        None => selection.clear(),
    }
}

/// A drag begins at `press` (screen): on the picked box's knob, handle or inside, or on another
/// annotation, which gets picked and moves.
fn start(
    editor: &mut EditorWindow,
    shown: &[OnFrame],
    picked: Option<&OnFrame>,
    map: Mapping,
    press: Pos2,
) {
    let at = map.canvas(press);
    let (grab, knob) = (map.length(GRAB), map.length(KNOB));
    let own = picked.and_then(|s| s.boxed.grip(at, grab, knob).map(|g| (s, g)));
    let Some((on, grip)) = own.or_else(|| topmost(shown, at).map(|s| (s, Grip::Move))) else {
        return;
    };
    if picked.map(|s| s.edit.id) != Some(on.edit.id) {
        let item = Selected::Region(Item::of(on.edit.id));
        editor.view.selection.only(item);
    }
    let drag = boxes::Drag::new(on, grip, at, map.canvas);
    editor.view.stage = Some(Drag::Annot(drag));
}

/// The pointer at `p` (canvas px) during the drag: the box follows, the preview shows it.
fn drag(ui: &Ui, editor: &mut EditorWindow, shown: &[OnFrame], map: Mapping, p: Pos2) {
    let Some(Drag::Annot(drag)) = &editor.view.stage else {
        return;
    };
    let modifiers = ui.input(|i| i.modifiers);
    let lines = (editor.view.magnet && !modifiers.command).then(|| {
        let others: Vec<_> = shown
            .iter()
            .filter(|s| s.edit.id != drag.edit.id)
            .map(|s| s.boxed.bounds())
            .collect();
        Lines::new(map.canvas, &others)
    });
    let how = Resize {
        keep_aspect: modifiers.shift,
        from_centre: modifiers.alt,
    };
    let (now, guides) = drag.follow(p, how, lines.as_ref(), map);
    let Some(Drag::Annot(drag)) = &mut editor.view.stage else {
        return;
    };
    drag.guides = guides;
    if now != drag.now {
        drag.now = now;
        drag.drafted = true;
        let steps = vec![drag.step()];
        show_draft(ui, editor, &steps);
    }
}
