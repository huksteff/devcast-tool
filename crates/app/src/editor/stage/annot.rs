//! Annotations on the preview (7.22, mockup «Главное окно»: the blue box, 8 handles, the knob on
//! top). A click picks the topmost annotation under the pointer (paint order: track, then order
//! within it), a click on nothing drops the selection. The picked one's box drags, its handles
//! resize it (Shift keeps the proportion, Alt keeps the centre), its knob turns it (Shift: steps
//! of 15°); the edges and the centre stick to the frame and to the other annotations shown (7.23,
//! `snap.rs`) while the snapping (N) is on and Ctrl is up. One `annot set` per gesture, to the key
//! at the playhead when it stands on one (`--at`, the rule of the panel, 7.18); the preview shows
//! the draft meanwhile. A left drag inside the picked drawing draws instead (`brush.rs`); its box
//! moves with a right drag (mockup 3c).

use dc_fx_annotations::{AnnotId, Annotations, Kind, SIZE_MIN, key_offset};
use dc_ops::Step;
use eframe::egui::{CursorIcon, PointerButton, Pos2, Response, Ui, Vec2};
use serde_json::json;

use super::{Drag, brush, finish, show_draft};
use crate::editor::EditorWindow;
use crate::editor::selection::{Item, Selected};
use crate::editor::timeline::Row;
use dc_fx_annotations::panel::{Edit, target};
use dc_ui_kit::stage::geom::{Boxed, GRAB, Grip, KNOB, Mapping, Resize};
use dc_ui_kit::stage::paint;
use dc_ui_kit::stage::snap::{Guide, Lines, PULL, axes};

/// An annotation the playhead shows: where its edits go and its box now.
#[derive(Debug, Clone, PartialEq)]
pub struct OnFrame {
    pub edit: Edit,
    pub kind: Kind,
    pub boxed: Boxed,
}

/// A drag of the picked annotation's box: what it holds, the box and the press (canvas px), the
/// box now and the lines it sticks to.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotDrag {
    pub edit: Edit,
    grip: Grip,
    from: Boxed,
    at: Pos2,
    /// Canvas size, px: the box is written in fractions of it.
    canvas: Vec2,
    pub now: Boxed,
    pub guides: Vec<Guide>,
    /// The preview shows a draft of it.
    pub drafted: bool,
}

/// The annotations shown at the playhead, bottom to top, with their boxes on a `canvas` px
/// frame; hidden rows do not count.
pub fn shown(editor: &EditorWindow, canvas: Vec2) -> Vec<OnFrame> {
    let project = editor.doc.project();
    let Ok(state) = project.state::<Annotations>() else {
        return Vec::new();
    };
    let now = editor.view.playhead;
    let mut found = Vec::new();
    for track in editor.tracks.rows.iter().filter(|t| !t.off) {
        let Row::Annotations(row) = track.row else {
            continue;
        };
        for region in &track.regions {
            let Some(id) = region.item.and_then(Item::id) else {
                continue;
            };
            if now < region.range.start() || now >= region.range.end() {
                continue;
            }
            let Some(order) = state.annotations.iter().position(|a| a.id == id) else {
                continue;
            };
            let annotation = &state.annotations[order];
            let Ok(offset) = key_offset(project, annotation, now) else {
                continue;
            };
            let (rect, rotation) = annotation.frame_at(offset);
            let at = target(&region.keys, now, region.range, editor.view.auto_keys);
            let edit = Edit { id, at };
            let boxed = Boxed::of(rect, rotation, canvas);
            let kind = annotation.kind;
            found.push(((row, order), OnFrame { edit, kind, boxed }));
        }
    }
    found.sort_by_key(|(order, _)| *order);
    found.into_iter().map(|(_, on)| on).collect()
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
    let drawing = picked.filter(|s| s.kind == Kind::Drawing);
    let over_drawing =
        drawing.is_some() && held.is_none() && pointer.and_then(own) == Some(Grip::Move);
    match (drawing, pointer) {
        (Some(on), Some(at)) if brushing || over_drawing => brush::hover(ui, editor, on, map, at),
        _ => {
            if let Some(grip) = held.or(hovered) {
                let angle = picked.map_or(0.0, |s| s.boxed.angle);
                ui.ctx().set_cursor_icon(cursor(grip, angle));
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
            s.kind == Kind::Drawing
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
                let changed = drag.now != drag.from;
                finish(editor, drag.drafted, changed.then(|| vec![step(&drag)]));
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
    editor.view.stage = Some(Drag::Annot(AnnotDrag {
        edit: on.edit,
        grip,
        from: on.boxed,
        at,
        canvas: map.canvas,
        now: on.boxed,
        guides: Vec::new(),
        drafted: false,
    }));
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
    let (now, guides) = follow(drag, p, how, lines.as_ref(), map);
    let Some(Drag::Annot(drag)) = &mut editor.view.stage else {
        return;
    };
    drag.guides = guides;
    if now != drag.now {
        drag.now = now;
        drag.drafted = true;
        let steps = vec![step(drag)];
        show_draft(ui, editor, &steps);
    }
}

/// The box of `drag` with the pointer at `p`: moved, resized or turned; stuck to `lines`.
fn follow(
    drag: &AnnotDrag,
    p: Pos2,
    how: Resize,
    lines: Option<&Lines>,
    map: Mapping,
) -> (Boxed, Vec<Guide>) {
    let pull = map.length(PULL);
    let from = drag.from;
    match drag.grip {
        Grip::Move => {
            let moved = from.moved(p - drag.at);
            match lines {
                Some(lines) => {
                    let (shift, guides) = lines.moved(moved.bounds(), pull);
                    (moved.moved(shift), guides)
                }
                None => (moved, Vec::new()),
            }
        }
        Grip::Handle(handle) => {
            // The handle itself follows the pointer, wherever on it the press was.
            let mut pointer = p + (from.handle(handle) - drag.at);
            let mut guides = Vec::new();
            if let (Some(lines), Some(side)) = (lines, axes(from.angle, handle.side())) {
                (pointer, guides) = lines.pointer(pointer, side, pull);
            }
            let least = map.canvas * SIZE_MIN as f32;
            (from.resized(handle, pointer, how, least), guides)
        }
        Grip::Turn => {
            let angle = from.turned(drag.at, p, how.keep_aspect);
            (Boxed { angle, ..from }, Vec::new())
        }
    }
}

/// `annot set <id> …` of what the gesture changes, at the key when there is one.
fn step(drag: &AnnotDrag) -> Step {
    let [x, y, w, h] = drag.now.fractions(drag.canvas);
    match drag.grip {
        Grip::Move => drag
            .edit
            .step("annot set", [("x", json!(x)), ("y", json!(y))]),
        Grip::Handle(_) => drag.edit.step(
            "annot set",
            [
                ("x", json!(x)),
                ("y", json!(y)),
                ("w", json!(w)),
                ("h", json!(h)),
            ],
        ),
        Grip::Turn => drag
            .edit
            .step("annot set", [("rotation", json!(drag.now.rotation()))]),
    }
}

/// The cursor over `grip` of a box turned `angle` degrees.
fn cursor(grip: Grip, angle: f32) -> CursorIcon {
    let Grip::Handle(handle) = grip else {
        return match grip {
            Grip::Turn => CursorIcon::Grab,
            _ => CursorIcon::Move,
        };
    };
    let side = handle.side();
    let bearing = side.y.atan2(side.x).to_degrees() + angle;
    let eighth = ((bearing.rem_euclid(180.0) / 45.0).round() as i64).rem_euclid(4);
    match eighth {
        0 => CursorIcon::ResizeHorizontal,
        1 => CursorIcon::ResizeNwSe,
        2 => CursorIcon::ResizeVertical,
        _ => CursorIcon::ResizeNeSw,
    }
}
