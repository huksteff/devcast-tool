//! The brush as a gesture on the preview (7.24, `dc_ui_kit::stage::Stage`): over the picked
//! drawing a left drag inside its box draws ([`Brushing`]): the stroke starts there, follows the
//! pointer (Alt: a line from the first point) and ends as one undo step. The circle under the
//! pointer shows its thickness; the bar over the preview chooses the tool and the eraser's size
//! ([`Brush`], kept by the window, not the project). It comes before the annotations' boxes in
//! the editor's table: they leave such a drag to it.

use dc_ui_kit::egui::{PointerButton, Pos2, Rect, Response, Ui};
use dc_ui_kit::stage::geom::{GRAB, Grip, KNOB, Mapping};
use dc_ui_kit::stage::{Answer, Cx, Gesture, Held, Stage};

use dc_fx_annotations::stage::{OnFrame, on_frame, picked};

use crate::stage::{Brush, Brushing, Picked, bar, hover, is_drawing, pick};

/// The entry of the editor's table of preview gestures.
pub struct Strokes;

impl Gesture for Brushing {
    fn drafted(&self) -> bool {
        self.drafted
    }
}

impl Stage for Strokes {
    fn ui(&self, ui: &Ui, cx: &mut Cx<'_>, response: &Response, map: Mapping) -> Answer {
        let mut answer = Answer::default();
        let shown = on_frame(cx, map.canvas);
        let id = picked(cx);
        let Some(on) = shown.iter().find(|s| Some(s.edit.id) == id && s.draws()) else {
            return answer;
        };
        let (grab, knob) = (map.length(GRAB), map.length(KNOB));
        let inside = |p: Pos2| on.boxed.grip(map.canvas(p), grab, knob) == Some(Grip::Move);
        let brushing = mine(cx).map(|stroke| stroke.width);
        let free = cx.held.is_none();
        if let Some(at) = response.hover_pos()
            && (brushing.is_some() || (free && inside(at)))
        {
            let shift = ui.input(|i| i.modifiers.shift);
            if let Some(width) = brushing.or_else(|| chosen(cx, on, shift).map(|(_, _, w)| w)) {
                hover(ui, map, at, width);
            }
        }
        if response.drag_started_by(PointerButton::Primary)
            && let Some(press) = ui.input(|i| i.pointer.press_origin())
            && inside(press)
        {
            let shift = ui.input(|i| i.modifiers.shift);
            if let Some(picked) = chosen(cx, on, shift) {
                *cx.held = Some(Held::new(Brushing::new(on, picked, map.canvas(press))));
            }
        }
        if response.dragged()
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(stroke) = cx.held.as_mut().and_then(Held::get_mut::<Brushing>)
            && stroke.add(map.canvas(pointer), ui.input(|i| i.modifiers.alt))
        {
            answer.draft = Some(vec![stroke.step()]);
        }
        if response.drag_stopped()
            && let Some(stroke) = mine(cx)
        {
            answer.done = Some(Some(vec![stroke.step()]));
        }
        answer
    }

    fn bar(&self, ui: &Ui, cx: &mut Cx<'_>, area: Rect) {
        let Some(id) = picked(cx) else {
            return;
        };
        if !is_drawing(cx.project, id) {
            return;
        }
        let mut chosen = cx.kept.get::<Brush>();
        bar(ui, &mut chosen, area);
        cx.kept.set(chosen);
    }
}

/// The stroke going on.
fn mine<'c>(cx: &'c Cx<'_>) -> Option<&'c Brushing> {
    cx.held.as_ref().and_then(Held::get::<Brushing>)
}

/// Tool, colour and thickness of a stroke of drawing `on` begun now, Shift held or not.
fn chosen(cx: &Cx<'_>, on: &OnFrame, shift: bool) -> Option<Picked> {
    pick(cx.project, (on, cx.playhead), cx.kept.get::<Brush>(), shift)
}
