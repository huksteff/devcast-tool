//! Annotations on the preview (7.22, `dc_ui_kit::stage`, mockup «Главное окно»: the blue box, 8
//! handles, the knob on top): which annotations the playhead shows with their boxes ([`shown`],
//! paint order: track, then order within it), and what a drag of the picked box makes of it —
//! moved, resized (Shift keeps the proportion, Alt keeps the centre) or turned (Shift: steps of
//! 15°), stuck to the lines it is given ([`Drag::follow`]). One `annot set` per gesture, to the
//! key the panel would edit ([`Edit`], `--at`). Picking and the gestures are the editor's
//! table entry [`Boxes`] (`boxes.rs`).

use dc_core::OpName;
use dc_core::{OutputTime, Project, Step};
use dc_ui_kit::egui::{CursorIcon, Pos2, Vec2};
use dc_ui_kit::stage::geom::{Boxed, Grip, Mapping, Resize};
use dc_ui_kit::stage::snap::{Guide, Lines, PULL, axes};
use dc_ui_kit::stage::{Cx, Shown};
use serde_json::json;

pub use crate::boxes::Boxes;
pub use crate::panel::Edit;
use crate::panel::target;
use crate::{AnnotId, AnnotKind, Annotations, Kind, SIZE_MIN, key_offset};

const ANNOT_SET: OpName = OpName::of::<crate::AnnotSet>();

/// An annotation the playhead shows: where its edits go and its box now.
#[derive(Debug, Clone, PartialEq)]
pub struct OnFrame {
    pub edit: Edit,
    pub kind: Kind,
    pub boxed: Boxed,
}

impl OnFrame {
    /// A drawing: a left drag inside its box draws.
    pub fn draws(&self) -> bool {
        self.kind == Kind::Drawing
    }

    /// A text: a double-click puts the keys into its text on the panel (its field is keyed by
    /// the annotation's id, `dc_ui_kit::panel::rows::focus_text`).
    pub fn typed(&self) -> bool {
        self.kind == Kind::Text
    }
}

/// The annotations of `placed` (the regions shown) the `playhead` shows, bottom to top, with their
/// boxes on a `canvas` px frame; `auto_keys` — where their edits go.
fn shown<'a>(
    project: &Project,
    (playhead, auto_keys): (OutputTime, bool),
    placed: impl IntoIterator<Item = &'a Shown<'a>>,
    canvas: Vec2,
) -> Vec<OnFrame> {
    let Ok(state) = project.state::<Annotations>() else {
        return Vec::new();
    };
    let now = playhead;
    let mut found = Vec::new();
    for region in placed {
        let Some(id) = region.item.id::<AnnotKind>() else {
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
        let at = target(region.keys, now, region.range, auto_keys);
        let edit = Edit { id, at };
        let boxed = Boxed::of(rect, rotation, canvas);
        let kind = annotation.kind;
        found.push(((region.track, order), OnFrame { edit, kind, boxed }));
    }
    found.sort_by_key(|(order, _)| *order);
    found.into_iter().map(|(_, on)| on).collect()
}

/// The annotations of the regions `cx` shows at its playhead, bottom to top, with their boxes on a
/// `canvas` px frame.
pub fn on_frame(cx: &Cx<'_>, canvas: Vec2) -> Vec<OnFrame> {
    shown(cx.project, (cx.playhead, cx.auto_keys), cx.shown, canvas)
}

/// The annotation picked alone (as a region or by one of its keys).
pub fn picked(cx: &Cx<'_>) -> Option<AnnotId> {
    cx.picked?.item().id::<AnnotKind>()
}

/// A drag of the picked annotation's box: what it holds, the box and the press (canvas px), the
/// box now and the lines it sticks to.
#[derive(Debug, Clone, PartialEq)]
pub struct Drag {
    pub edit: Edit,
    pub grip: Grip,
    from: Boxed,
    at: Pos2,
    /// Canvas size, px: the box is written in fractions of it.
    canvas: Vec2,
    pub now: Boxed,
    pub guides: Vec<Guide>,
    /// The preview shows a draft of it.
    pub drafted: bool,
}

impl Drag {
    /// A drag of `on` held by `grip`, pressed at `at` (canvas px) of a `canvas` px frame.
    pub fn new(on: &OnFrame, grip: Grip, at: Pos2, canvas: Vec2) -> Self {
        Self {
            edit: on.edit,
            grip,
            from: on.boxed,
            at,
            canvas,
            now: on.boxed,
            guides: Vec::new(),
            drafted: false,
        }
    }

    /// The box moved away from where it was pressed.
    pub fn changed(&self) -> bool {
        self.now != self.from
    }

    /// The box with the pointer at `p` (canvas px): moved, resized or turned; stuck to `lines`.
    pub fn follow(
        &self,
        p: Pos2,
        how: Resize,
        lines: Option<&Lines>,
        map: Mapping,
    ) -> (Boxed, Vec<Guide>) {
        let pull = map.length(PULL);
        let from = self.from;
        match self.grip {
            Grip::Move => {
                let moved = from.moved(p - self.at);
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
                let mut pointer = p + (from.handle(handle) - self.at);
                let mut guides = Vec::new();
                if let (Some(lines), Some(side)) = (lines, axes(from.angle, handle.side())) {
                    (pointer, guides) = lines.pointer(pointer, side, pull);
                }
                let least = map.canvas * SIZE_MIN as f32;
                (from.resized(handle, pointer, how, least), guides)
            }
            Grip::Turn => {
                let angle = from.turned(self.at, p, how.keep_aspect);
                (Boxed { angle, ..from }, Vec::new())
            }
        }
    }

    /// `annot set <id> …` of what the gesture changes, at the key when there is one.
    pub fn step(&self) -> Step {
        let [x, y, w, h] = self.now.fractions(self.canvas);
        match self.grip {
            Grip::Move => self
                .edit
                .step(ANNOT_SET, [("x", json!(x)), ("y", json!(y))]),
            Grip::Handle(_) => self.edit.step(
                ANNOT_SET,
                [
                    ("x", json!(x)),
                    ("y", json!(y)),
                    ("w", json!(w)),
                    ("h", json!(h)),
                ],
            ),
            Grip::Turn => self
                .edit
                .step(ANNOT_SET, [("rotation", json!(self.now.rotation()))]),
        }
    }
}

/// The cursor over `grip` of a box turned `angle` degrees.
pub fn cursor(grip: Grip, angle: f32) -> CursorIcon {
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
