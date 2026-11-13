//! Cues (mockup 3b «Реплики»): the list of the captions tab and the editor of one cue — the
//! same rows in the list (under the picked cue) and in the cue's own panel when it is picked on
//! the timeline ([`cue`]). Text and times are `captions set` (Enter or a click away applies),
//! «Разделить» `captions split` at the playhead, «Объединить» `captions merge`, the bin
//! `captions rm`; each one undo step. A click on a line of the list asks the editor to pick the
//! cue (`Request::Pick`, the list stays open) and to move the playhead to its start
//! (`Request::Seek`).

use dc_core::OpName;
use dc_core::time::Output;
use dc_core::{OutputTime, Range, Step};
use dc_ui_kit::egui::{self, Align, Layout, Rect, Sense, Ui, WidgetInfo, pos2, vec2};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{Cx, Request, args, header_button, padded, rows};
use dc_ui_kit::strings::{captions as text, inspector as names};
use dc_ui_kit::tokens::{ACCENT_SOFT, FS_M, HUE_CAPTIONS, S1, S2, TEXT, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{Button, Kind};
use serde_json::{Value, json};

use crate::panel::{List, Shown};
use crate::{CaptionsEffect, CaptionsState, Cue, CueId};

const CAPTIONS_MERGE: OpName = OpName::of::<crate::CaptionsMerge>();
const CAPTIONS_RM: OpName = OpName::of::<crate::CaptionsRm>();
const CAPTIONS_SPLIT: OpName = OpName::of::<crate::CaptionsSplit>();

const SET: OpName = OpName::of::<crate::CaptionsSet>();
/// Width of the time column of the list, its font size, the padding of a line, points.
const TIME: f32 = 58.0;
const TIME_SIZE: f32 = 11.0;
const PAD: f32 = 6.0;

/// The panel of cue `id` picked on the timeline, shown at `at` of the output: its text and
/// times, split, merge (the frame of the editor gives it its header with «Удалить»).
pub fn cue(ui: &mut Ui, cx: &mut Cx<'_>, id: CueId, at: Range<Output>) {
    let Ok(state) = cx.project.state::<CaptionsEffect>() else {
        return;
    };
    let Some(found) = state.cues.iter().find(|c| c.id == id) else {
        return;
    };
    padded(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        editor(ui, cx, (&state, found), at, false);
        rows::note(ui, text::CUE_NOTE);
    });
}

/// The list; the picked cue opens its editor under its line.
pub fn list(ui: &mut Ui, cx: &mut Cx<'_>, state: &CaptionsState, list: &List) {
    let playhead = cx.playhead;
    for shown in &list.shown {
        let Some(cue) = state.cues.iter().find(|c| c.id == shown.id) else {
            continue;
        };
        let picked = list.picked == Some(shown.id);
        let now = shown.at.start() <= playhead && playhead < shown.at.end();
        if line(ui, cue, shown, picked, now) {
            cx.requests.push(Request::Pick(shown.id.to_string()));
            cx.requests.push(Request::Seek(shown.at.start()));
        }
        if picked {
            editor(ui, cx, (state, cue), shown.at, true);
        }
    }
    rows::note(ui, text::CUE_NOTE);
}

/// One line of the list: its time and text; true when clicked.
fn line(ui: &mut Ui, cue: &Cue, shown: &Shown, picked: bool, now: bool) -> bool {
    let said = cue.text();
    let width = ui.available_width();
    let color = if picked || now { TEXT } else { TEXT_2 };
    let galley = ui.painter().layout(
        said.clone(),
        fonts::text(FS_M, Weight::Regular),
        color,
        width - TIME - 2.0 * PAD,
    );
    let size = vec2(width, galley.size().y + 2.0 * PAD);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let name = text::cue_line(&shown.start, &said);
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, picked, &name));
    let painter = ui.painter();
    if picked {
        painter.rect_filled(rect, 4, ACCENT_SOFT);
    }
    if picked || now {
        let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
        painter.rect_filled(bar, 1, HUE_CAPTIONS.line);
    }
    let time = pos2(rect.left() + PAD, rect.top() + PAD + 2.0);
    let font = fonts::mono(TIME_SIZE);
    painter.text(time, egui::Align2::LEFT_TOP, &shown.start, font, TEXT_3);
    let words = pos2(rect.left() + TIME + PAD, rect.top() + PAD);
    painter.galley(words, galley, color);
    response.clicked()
}

/// The editor of `cue` of `state` (shown at `at` of the output): text, start and end, split,
/// merge and, in the list, delete (the cue's own panel has «Удалить» in its header).
fn editor(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    (state, cue): (&CaptionsState, &Cue),
    at: Range<Output>,
    listed: bool,
) {
    let id = cue.id;
    let said = cue.text();
    let apply = |typed: String| set(id, "text", json!(typed));
    if let Some(step) = rows::text(ui, text::CUE_TEXT, ("cue-text", id), &said, apply) {
        cx.edits.push(vec![step]);
    }
    ui.horizontal(|ui| {
        let half = (ui.available_width() - S2) / 2.0;
        for (field, label, now) in [
            ("from", text::CUE_START, at.start()),
            ("to", text::CUE_END, at.end()),
        ] {
            let shown = now.to_string();
            let apply = |typed: String| set(id, field, json!(typed));
            let typed = ui
                .allocate_ui(vec2(half, ui.spacing().interact_size.y), |ui| {
                    rows::text(ui, label, ("cue-time", id, field), &shown, apply)
                })
                .inner;
            if let Some(step) = typed {
                cx.edits.push(vec![step]);
            }
        }
    });
    buttons(ui, cx, (state, cue), listed);
}

fn buttons(ui: &mut Ui, cx: &mut Cx<'_>, (state, cue): (&CaptionsState, &Cue), listed: bool) {
    let id = cue.id;
    let next = state
        .cues
        .iter()
        .position(|c| c.id == id)
        .and_then(|i| state.cues.get(i + 1))
        .is_some_and(|n| n.media == cue.media);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = S1;
        let split = Button::new(text::SPLIT)
            .kind(Kind::Ghost)
            .small()
            .icon(Icon::Blade);
        let response = ui.add_enabled(cue.words.len() > 1, split);
        if response.on_hover_text(text::SPLIT_HINT).clicked() {
            let at = seconds(cx.playhead);
            cx.apply(CAPTIONS_SPLIT, [("id", json!(id)), ("at", json!(at))]);
        }
        let merge = Button::new(text::MERGE).kind(Kind::Ghost).small();
        let response = ui.add_enabled(next, merge);
        if response.on_hover_text(text::MERGE_HINT).clicked() {
            cx.apply(CAPTIONS_MERGE, [("id", json!(id))]);
        }
        if !listed {
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if header_button(ui, Icon::Trash, names::DELETE, true) {
                cx.apply(CAPTIONS_RM, [("id", json!(id))]);
            }
        });
    });
}

/// Output seconds as `devcast` writes them: six digits of microseconds.
fn seconds(t: OutputTime) -> f64 {
    t.micros() as f64 / 1e6
}

/// `captions set <id> --<field> <value>`.
fn set(id: CueId, field: &'static str, value: Value) -> Step {
    Step::of(SET, args([("id", json!(id)), (field, value)]))
}
