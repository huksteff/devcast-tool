//! The panel of a callout event picked on the «Курсор» row (mockup 3a «Облачко: текст»).
//! «Событие» — the action «Вкл · Текст · Выкл», the text and how long typing or erasing takes
//! (`callout set`); the text of «Вкл» is the project's one for every «Вкл» (`callout style
//! --text`). «Стиль облачка» — the look of every callout of the project (`callout style`), its
//! «Сбросить» brings the look (not the text of «Вкл») to the defaults with one undo step.

use dc_fx_callout::{
    Action as Kind, Callout, CalloutEvent, CalloutSettings, EventId, GLOW_MAX, SIZE_MAX, SIZE_MIN,
    TEXT_MAX,
};
use dc_ops::Step;
use dc_render::Rgba;
use dc_ui_kit::strings::{inspector as names, timeline as rows_text};
use eframe::egui::Ui;
use serde_json::{Map, Value, json};

use super::{Cx, Group, args, padded, reset, rows};
use crate::editor::Action;

/// Id of the callout panel in the remembered groups.
pub const ID: &str = "callout";
const SET: &str = "callout set";
const STYLE: &str = "callout style";
/// Bounds of the typing or erasing time on its slider, seconds; the command takes any length.
const TIME_MIN: f64 = 0.1;
const TIME_MAX: f64 = 10.0;

/// The actions in the order of the segmented row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Does {
    On,
    Text,
    Off,
}

const ACTIONS: [(Does, &str); 3] = [
    (Does::On, names::CALLOUT_SHOW),
    (Does::Text, names::CALLOUT_TYPE),
    (Does::Off, names::CALLOUT_HIDE),
];

/// The event `id` of the project and the look of every callout.
fn read(cx: &Cx<'_>, id: EventId) -> Option<(CalloutEvent, CalloutSettings)> {
    let state = cx.base.project.state::<Callout>().ok()?;
    let event = state.events.into_iter().find(|e| e.id == id)?;
    Some((event, state.settings))
}

/// The title and the subtitle of the header of event `id` that starts at `at` of the output.
pub fn about(cx: &Cx<'_>, id: EventId, at: &str) -> Option<(&'static str, String)> {
    let (event, _) = read(cx, id)?;
    let (title, what) = match event.action {
        Kind::On => (names::CALLOUT_ON, names::CALLOUT_LENGTH),
        Kind::Text { .. } => (names::CALLOUT_TEXT, names::CALLOUT_TYPING),
        Kind::Off => (names::CALLOUT_OFF, names::CALLOUT_ERASING),
    };
    let length = rows_text::seconds(event.duration.as_secs_f64());
    Some((title, names::callout_about(at, what, &length)))
}

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, id: EventId) {
    let Some((event, now)) = read(cx, id) else {
        return;
    };
    let default = match cx.defaults_of::<Callout>() {
        Ok(state) => state.settings,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    cx.base.groups.panel(ID);
    let group = Group {
        id: "event",
        title: names::CALLOUT_EVENT,
        open: true,
        reset: None,
    };
    cx.group(ui, group, |ui, cx| event_rows(ui, cx, &event, &now));
    let group = Group {
        id: "style",
        title: names::CALLOUT_STYLE,
        open: true,
        reset: Some(
            reset(STYLE, &look_args(&now), look_args(&default))
                .into_iter()
                .collect(),
        ),
    };
    cx.group(ui, group, |ui, cx| style_rows(ui, cx, &now));
    padded(ui, |ui| rows::note(ui, names::CALLOUT_SHARED));
}

fn event_rows(ui: &mut Ui, cx: &mut Cx<'_>, event: &CalloutEvent, now: &CalloutSettings) {
    let id = event.id;
    let does = match &event.action {
        Kind::On => Does::On,
        Kind::Text { .. } => Does::Text,
        Kind::Off => Does::Off,
    };
    if let Some(new) = rows::choice(ui, names::CALLOUT_ACTION, Some(does), &ACTIONS) {
        cx.actions.push(switch(id, new, now));
    }
    match &event.action {
        Kind::On => {
            let apply = |text: String| step(STYLE, [("text", json!(text))]);
            if let Some(step) = rows::text(ui, names::CALLOUT_TEXT_HINT, id, &now.text, apply) {
                cx.actions.push(Action::Apply(vec![step]));
            }
            rows::note(ui, names::CALLOUT_ON_NOTE);
        }
        Kind::Text { text } => {
            let apply = |new: String| step(SET, [("id", json!(id)), ("text", json!(new))]);
            if let Some(step) = rows::text(ui, names::CALLOUT_TEXT_HINT, id, text, apply) {
                cx.actions.push(Action::Apply(vec![step]));
            }
            rows::note(ui, &names::callout_count(text.chars().count(), TEXT_MAX));
            time_row(ui, cx, event, names::CALLOUT_TIME);
        }
        Kind::Off => {
            rows::note(ui, names::CALLOUT_OFF_NOTE);
            time_row(ui, cx, event, names::CALLOUT_ERASE_TIME);
        }
    }
}

fn time_row(ui: &mut Ui, cx: &mut Cx<'_>, event: &CalloutEvent, label: &str) {
    let seconds = event.duration.as_secs_f64();
    let step = (0.1, names::SECONDS);
    if let Some(v) = rows::slider(ui, label, seconds, TIME_MIN..=TIME_MAX, step) {
        cx.actions.push(set(event.id, [("duration", json!(v))]));
    }
}

/// The event becomes `does`; a «Текст» gets the text of «Вкл», or a word when there is none.
fn switch(id: EventId, does: Does, now: &CalloutSettings) -> Action {
    match does {
        Does::On => set(id, [("action", json!("on"))]),
        Does::Off => set(id, [("action", json!("off"))]),
        Does::Text => {
            let text = match now.text.as_str() {
                "" => names::CALLOUT_NEW_TEXT,
                text => text,
            };
            set(id, [("action", json!("text")), ("text", json!(text))])
        }
    }
}

fn style_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &CalloutSettings) {
    let colors = [
        (names::CALLOUT_BACKGROUND, now.background),
        (names::CALLOUT_TEXT_COLOR, now.text_color),
    ];
    if let Some((i, color)) = rows::colors(ui, names::CALLOUT_COLORS, &colors) {
        let field = if i == 0 { "background" } else { "text_color" };
        cx.actions.push(style(field, json!(color)));
    }
    let size = (0.05, names::TIMES);
    if let Some(v) = rows::slider(ui, names::CALLOUT_SIZE, now.size, SIZE_MIN..=SIZE_MAX, size) {
        cx.actions.push(style("size", json!(v)));
    }
    if let Some(on) = rows::toggle(ui, names::CALLOUT_GRADIENT, now.gradient) {
        cx.actions.push(style("gradient", json!(on)));
    }
    if now.gradient
        && let Some(colors) = rows::gradient(ui, names::CALLOUT_GRADIENT, now.colors.colors())
    {
        cx.actions.push(style("colors", json!(joined(&colors))));
    }
    let glow = (1.0, "");
    if let Some(v) = rows::slider(ui, names::CALLOUT_GLOW, now.glow, 0.0..=GLOW_MAX, glow) {
        cx.actions.push(style("glow", json!(v)));
    }
}

/// `<op> --<field> <value> …`.
fn step<const N: usize>(op: &str, fields: [(&'static str, Value); N]) -> Step {
    Step::new(op, args(fields))
}

/// `callout set <id> --<field> <value> …`.
fn set<const N: usize>(id: EventId, fields: [(&'static str, Value); N]) -> Action {
    Action::op(SET, std::iter::once(("id", json!(id))).chain(fields))
}

/// `callout style --<field> <value>`.
fn style(field: &'static str, value: Value) -> Action {
    Action::op(STYLE, [(field, value)])
}

/// `#14b8c4ff,#a855f7ff`: the colours as `callout style --colors` takes them.
fn joined(colors: &[Rgba]) -> String {
    let all: Vec<String> = colors.iter().map(Rgba::to_string).collect();
    all.join(",")
}

/// The look of every callout as arguments of `callout style`; the text of «Вкл» is not a look.
fn look_args(s: &CalloutSettings) -> Map<String, Value> {
    args([
        ("background", json!(s.background)),
        ("text_color", json!(s.text_color)),
        ("size", json!(s.size)),
        ("gradient", json!(s.gradient)),
        ("colors", json!(joined(s.colors.colors()))),
        ("glow", json!(s.glow)),
    ])
}
