//! The annotations' gesture on the preview (7.22, `dc_ui_kit::stage::Stage`): picking and the
//! gestures of the picked box. A click picks the topmost annotation under the pointer, a click on
//! nothing drops the selection. The picked one's box drags, its handles resize it, its knob turns
//! it ([`Drag`]); the edges and the centre stick to the frame and to the other annotations shown
//! (7.23) while the snapping (N) is on and Ctrl is up. One `annot set` per gesture; the preview
//! shows the draft meanwhile. A kind that is drawn on ([`OnFrame::draws`]) leaves a left drag
//! inside its picked box to its own stage, which comes before this one (its box moves with a
//! right drag, mockup 3c). A double-click on a text puts the keys into its text on the panel, all
//! of it selected, so typing replaces it.

use dc_ui_kit::egui::{PointerButton, Pos2, Response, Ui};
use dc_ui_kit::lane::Item;
use dc_ui_kit::panel::rows;
use dc_ui_kit::stage::geom::{GRAB, Grip, KNOB, Mapping, Resize};
use dc_ui_kit::stage::snap::Lines;
use dc_ui_kit::stage::{Answer, Cx, Gesture, Held, Pick, Stage, paint};

use crate::stage::{Drag, OnFrame, cursor, on_frame, picked};

/// The entry of the editor's table of preview gestures.
pub struct Boxes;

impl Gesture for Drag {
    fn drafted(&self) -> bool {
        self.drafted
    }
}

impl Stage for Boxes {
    fn ui(&self, ui: &Ui, cx: &mut Cx<'_>, response: &Response, map: Mapping) -> Answer {
        let shown = on_frame(cx, map.canvas);
        let picked = picked(cx).and_then(|id| shown.iter().find(|s| s.edit.id == id));
        hover(ui, cx, response, (&shown, picked), map);
        let mut answer = Answer::default();
        gestures(ui, cx, response, (&shown, picked), map, &mut answer);
        let id = picked.map(|s| s.edit.id);
        let (boxed, guides) = match mine(cx) {
            Some(drag) if Some(drag.edit.id) == id => (Some(drag.now), drag.guides.clone()),
            _ => (picked.map(|s| s.boxed), Vec::new()),
        };
        paint::guides(ui, map, &guides);
        if let Some(boxed) = boxed {
            paint::selected(ui, map, &boxed);
        }
        answer
    }
}

/// The box drag going on.
fn mine<'c>(cx: &'c Cx<'_>) -> Option<&'c Drag> {
    cx.held.as_ref().and_then(Held::get::<Drag>)
}

/// The cursor over the boxes; over the inside of a picked box that is drawn on, or while another
/// gesture goes on there, its own stage shows the pointer.
fn hover(
    ui: &Ui,
    cx: &Cx<'_>,
    response: &Response,
    (shown, picked): (&[OnFrame], Option<&OnFrame>),
    map: Mapping,
) {
    let (grab, knob) = (map.length(GRAB), map.length(KNOB));
    let held = mine(cx).map(|drag| drag.grip);
    let Some(pointer) = response.hover_pos() else {
        if let Some(grip) = held {
            set_cursor(ui, picked, grip);
        }
        return;
    };
    let own = |p: Pos2| picked.and_then(|s| s.boxed.grip(map.canvas(p), grab, knob));
    let other = cx.held.is_some() && held.is_none();
    let over_drawn = held.is_none() && own(pointer) == Some(Grip::Move);
    if picked.is_some_and(OnFrame::draws) && (other || over_drawn) {
        return;
    }
    let hovered = own(pointer).or_else(|| topmost(shown, map.canvas(pointer)).map(|_| Grip::Move));
    if let Some(grip) = held.or(hovered) {
        set_cursor(ui, picked, grip);
    }
}

fn set_cursor(ui: &Ui, picked: Option<&OnFrame>, grip: Grip) {
    let angle = picked.map_or(0.0, |s| s.boxed.angle);
    ui.ctx().set_cursor_icon(cursor(grip, angle));
}

/// The press, click, drag and release on the preview: pick, move, resize or turn.
fn gestures(
    ui: &Ui,
    cx: &mut Cx<'_>,
    response: &Response,
    (shown, picked): (&[OnFrame], Option<&OnFrame>),
    map: Mapping,
    answer: &mut Answer,
) {
    if response.drag_started()
        && let Some(press) = ui.input(|i| i.pointer.press_origin())
    {
        let (grab, knob) = (map.length(GRAB), map.length(KNOB));
        let drawn = picked.is_some_and(|s| {
            s.draws()
                && response.drag_started_by(PointerButton::Primary)
                && s.boxed.grip(map.canvas(press), grab, knob) == Some(Grip::Move)
        });
        if !drawn {
            start(cx, (shown, picked), map, press, answer);
        }
    }
    if response.clicked()
        && let Some(at) = response.interact_pointer_pos()
    {
        answer.pick = Some(match topmost(shown, map.canvas(at)) {
            Some(on) => Pick::Only(Item::of(on.edit.id)),
            None => Pick::Nothing,
        });
    }
    if response.double_clicked()
        && let Some(at) = response.interact_pointer_pos()
        && let Some(on) = topmost(shown, map.canvas(at)).filter(|on| on.typed())
    {
        // The panel of the picked text is drawn next pass: its field takes the keys then.
        rows::focus_text(ui.ctx(), on.edit.id);
        ui.ctx().request_repaint();
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        drag(ui, cx, shown, map, map.canvas(pointer), answer);
    }
    if response.drag_stopped()
        && let Some(drag) = mine(cx)
    {
        answer.done = Some(drag.changed().then(|| vec![drag.step()]));
    }
}

/// The topmost of `shown` under `p` (canvas px).
fn topmost(shown: &[OnFrame], p: Pos2) -> Option<&OnFrame> {
    shown.iter().rev().find(|s| s.boxed.contains(p))
}

/// A drag begins at `press` (screen): on the picked box's knob, handle or inside, or on another
/// annotation, which gets picked and moves.
fn start(
    cx: &mut Cx<'_>,
    (shown, picked): (&[OnFrame], Option<&OnFrame>),
    map: Mapping,
    press: Pos2,
    answer: &mut Answer,
) {
    let at = map.canvas(press);
    let (grab, knob) = (map.length(GRAB), map.length(KNOB));
    let own = picked.and_then(|s| s.boxed.grip(at, grab, knob).map(|g| (s, g)));
    let Some((on, grip)) = own.or_else(|| topmost(shown, at).map(|s| (s, Grip::Move))) else {
        return;
    };
    if picked.map(|s| s.edit.id) != Some(on.edit.id) {
        answer.pick = Some(Pick::Only(Item::of(on.edit.id)));
    }
    *cx.held = Some(Held::new(Drag::new(on, grip, at, map.canvas)));
}

/// The pointer at `p` (canvas px) during the drag: the box follows, the preview shows it.
fn drag(ui: &Ui, cx: &mut Cx<'_>, shown: &[OnFrame], map: Mapping, p: Pos2, answer: &mut Answer) {
    let magnet = cx.magnet;
    let Some(drag) = cx.held.as_mut().and_then(Held::get_mut::<Drag>) else {
        return;
    };
    let modifiers = ui.input(|i| i.modifiers);
    let lines = (magnet && !modifiers.command).then(|| {
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
    drag.guides = guides;
    if now != drag.now {
        drag.now = now;
        drag.drafted = true;
        answer.draft = Some(vec![drag.step()]);
    }
}
