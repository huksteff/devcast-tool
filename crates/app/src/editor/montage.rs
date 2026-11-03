//! Montage of the main track as registry commands of fx/cut, written as `devcast` takes them:
//! times are seconds with all six digits of microseconds, so the window and the CLI give the
//! same project. C, the blade, Delete, Shift+Delete, R, a dragged clip and the range menu ask
//! for them here.

use dc_core::time::Output;
use dc_core::{ClipId, OutputTime, Range};
use dc_ops::Step;
use serde_json::{Map, Value};

use super::view::RangeMenu;
use super::{Action, EditorWindow};

/// `t` as a time argument: `12.500000` (`dc_ui_kit::panel::time_arg`, as the panels write it).
pub fn arg(t: OutputTime) -> String {
    dc_ui_kit::panel::time_arg(t)
}

/// `range` as a range argument: `10.000000-12.500000`.
pub fn range_arg(range: Range<Output>) -> String {
    format!("{}-{}", arg(range.start()), arg(range.end()))
}

/// `clip split --at <at>`: the blade and C.
pub fn split(at: OutputTime) -> Action {
    Action::op("clip split", [("at", Value::String(arg(at)))])
}

/// `clip move <id> --to <place>`, `place` from 1.
pub fn move_to(id: ClipId, place: usize) -> Action {
    Action::op(
        "clip move",
        [("id", Value::String(id.to_string())), ("to", place.into())],
    )
}

/// `clip rm` of each clip in one undo step; nothing when there is none.
pub fn remove(ids: impl IntoIterator<Item = ClipId>) -> Option<Action> {
    let steps: Vec<Step> = ids
        .into_iter()
        .map(|id| {
            let mut args = Map::new();
            args.insert("id".to_owned(), Value::String(id.to_string()));
            Step::new("clip rm", args)
        })
        .collect();
    (!steps.is_empty()).then_some(Action::Apply(steps))
}

/// `cut <range>`: the range goes and the rest closes up (ripple).
pub fn cut(range: Range<Output>) -> Action {
    Action::op("cut", [("ranges", Value::String(range_arg(range)))])
}

/// `speed <range> <factor>`.
pub fn speed(range: Range<Output>, factor: f64) -> Action {
    Action::op(
        "speed",
        [
            ("range", Value::String(range_arg(range))),
            ("factor", factor.into()),
        ],
    )
}

/// C: cuts the clip under the playhead; the left half keeps the clip and its selection.
pub fn cut_at_playhead(editor: &mut EditorWindow) {
    editor.ask(split(editor.view.playhead));
}

/// Removes the picked clips.
pub fn delete(editor: &mut EditorWindow) {
    if let Some(action) = remove(editor.view.selection.clips().collect::<Vec<_>>()) {
        editor.ask(action);
    }
}

/// Shift+Delete: cuts the picked range out with ripple; without a range, removes the picked
/// clips (the track has no gaps, so that is a ripple delete too).
pub fn cut_range(editor: &mut EditorWindow) {
    match editor.view.range.take() {
        Some(range) => {
            editor.view.menu = None;
            editor.ask(cut(range));
        }
        None => delete(editor),
    }
}

/// R: opens the speed popover of the picked range.
pub fn range_speed(editor: &mut EditorWindow) {
    if editor.view.range.is_some() {
        editor.view.menu = Some(RangeMenu::Speed { own: 1.0 });
    }
}

/// Esc: closes the range menu, else drops dragged regions or a clip where they were, else the
/// range, else takes «Выбор» back from «Лезвие».
pub fn escape(editor: &mut EditorWindow) {
    if super::stage::cancel(editor) {
        return;
    }
    let view = &mut editor.view;
    let gesture = view.gesture.take().is_some();
    if gesture
        || view.remove.take().is_some()
        || view.lift.take().is_some()
        || view.menu.take().is_some()
        || view.drag.take().is_some()
        || view.range.take().is_some()
    {
        view.ranging = None;
        return;
    }
    view.tool = super::view::Tool::Select;
}
