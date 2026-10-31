//! Cues (mockup 3b «Реплики»): the list of the captions panel and the editor of one cue — the
//! same rows in the list (under the picked cue) and in the cue's own panel when it is picked on
//! the timeline. Text and times are `captions set` (Enter or a click away applies), «Разделить»
//! `captions split` at the playhead, «Объединить» `captions merge`, the bin `captions rm`; each
//! one undo step. A click on a cue of the list picks it and moves the playhead to its start; the
//! list stays open (the pick does not switch the inspector to the cue's panel).

use dc_core::time::Output;
use dc_core::{OutputTime, Range};
use dc_fx_captions::{CaptionsState, Cue, CueId};
use dc_ops::Step;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{captions as text, inspector as names};
use dc_ui_kit::tokens::{ACCENT_SOFT, FS_M, HUE_CAPTIONS, S1, S2, TEXT, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{Button, Kind};
use eframe::egui::{self, Align, Layout, Rect, Sense, Ui, WidgetInfo, pos2, vec2};
use serde_json::{Value, json};

use super::{Cx, args, header_button, rows};
use crate::editor::selection::{Item, Selected};
use crate::editor::timeline::{Row, timecode};
use crate::editor::{Action, Transport};

const SET: &str = "captions set";
/// Width of the time column of the list, its font size, the padding of a line, points.
const TIME: f32 = 58.0;
const TIME_SIZE: f32 = 11.0;
const PAD: f32 = 6.0;

/// The cues the timeline shows (not cut out), in output order.
pub fn shown(cx: &Cx<'_>) -> Vec<(CueId, Range<Output>)> {
    let mut all: Vec<(CueId, Range<Output>)> = cx
        .tracks
        .rows
        .iter()
        .filter(|t| t.row == Row::Captions)
        .flat_map(|t| &t.regions)
        .filter_map(|r| match r.item {
            Some(Item::Cue(id)) => Some((id, r.range)),
            _ => None,
        })
        .collect();
    all.sort_by_key(|(_, range)| range.start());
    all
}

/// The list; the picked cue opens its editor under its line.
pub fn list(ui: &mut Ui, cx: &mut Cx<'_>, state: &CaptionsState) {
    let playhead = cx.captioning.playhead;
    for (id, at) in shown(cx) {
        let Some(cue) = state.cues.iter().find(|c| c.id == id) else {
            continue;
        };
        let picked = cx.captioning.picked == Some(id);
        let now = at.start() <= playhead && playhead < at.end();
        if line(ui, cue, at, picked, now) {
            let item = Selected::Region(Item::Cue(id));
            cx.actions.push(Action::ListPick(item));
            cx.actions
                .push(Action::Transport(Transport::Scrub(at.start())));
        }
        if picked {
            editor(ui, cx, (state, cue), at, true);
        }
    }
    rows::note(ui, text::CUE_NOTE);
}

/// One line of the list: its time and text; true when clicked.
fn line(ui: &mut Ui, cue: &Cue, at: Range<Output>, picked: bool, now: bool) -> bool {
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
    let start = timecode(at.start());
    let name = text::cue_line(&start, &said);
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
    painter.text(time, egui::Align2::LEFT_TOP, start, font, TEXT_3);
    let words = pos2(rect.left() + TIME + PAD, rect.top() + PAD);
    painter.galley(words, galley, color);
    response.clicked()
}

/// The editor of `cue` of `state` (shown at `at` of the output): text, start and end, split,
/// merge and, in the list, delete (the cue's own panel has «Удалить» in its header).
pub fn editor(
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
        cx.actions.push(Action::Apply(vec![step]));
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
                cx.actions.push(Action::Apply(vec![step]));
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
            let at = seconds(cx.captioning.playhead);
            cx.actions
                .push(op("captions split", id, [("at", json!(at))]));
        }
        let merge = Button::new(text::MERGE).kind(Kind::Ghost).small();
        let response = ui.add_enabled(next, merge);
        if response.on_hover_text(text::MERGE_HINT).clicked() {
            cx.actions.push(op("captions merge", id, []));
        }
        if !listed {
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if header_button(ui, Icon::Trash, names::DELETE, true) {
                cx.actions.push(op("captions rm", id, []));
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
    Step::new(SET, args([("id", json!(id)), (field, value)]))
}

/// `<op> <id> --<field> <value> …`.
fn op<const N: usize>(name: &str, id: CueId, fields: [(&'static str, Value); N]) -> Action {
    Action::op(name, std::iter::once(("id", json!(id))).chain(fields))
}
