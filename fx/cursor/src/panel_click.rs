//! The panel of one click picked on the «Курсор» row (mockup 3a «Клик»): the head says when and
//! which button; «Эффект этого клика» shows the click's effect — its own fields over the effect
//! of every click — and each edit is `click set --at <output time>` (the change of this press,
//! `ClickChange`); «Сбросить» is `click reset`: the click is like the others again. The
//! «Выкл» tile hides the effect of this click. A press is the recording's: it cannot be removed.
//! The press comes from the editor's dots of the row as [`Press`]: the effect does not read the
//! telemetry.

use dc_core::OpName;
use dc_core::{OutputTime, SourceTime, Step};
use dc_ui_kit::egui::Ui;
use dc_ui_kit::panel::{Cx, Group, Header, header, padded, rows, time_arg};
use dc_ui_kit::strings::inspector as names;
use dc_ui_kit::tokens::HUE_CURSOR;
use serde_json::{Map, Value, json};

use crate::Cursor;
use crate::panel::ID;
use crate::panel_style as style;

const CLICK_RESET: OpName = OpName::of::<crate::ClickReset>();
const CLICK_SET: OpName = OpName::of::<crate::ClickSet>();

/// A press of a recording as the «Курсор» row shows it: the facts of [`click`].
#[derive(Debug, Clone, PartialEq)]
pub struct Press {
    /// Where it is on the output.
    pub at: OutputTime,
    /// The recording (`Media.path`) and the press on its time: the key of a `ClickChange`.
    pub media: String,
    pub source: SourceTime,
    pub button: PressButton,
    /// The second press of a double click.
    pub double: bool,
}

/// The mouse button of a [`Press`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// The panel of `press`; `timecode` is its time as the timeline writes it.
pub fn click(ui: &mut Ui, cx: &mut Cx<'_>, press: &Press, timecode: &str) {
    let subtitle = names::click_about(timecode, button(press.button), press.double);
    let head = Header {
        hue: HUE_CURSOR,
        title: names::CLICK,
        subtitle: &subtitle,
    };
    header(ui, &head, |_| {});
    let state = match cx.project.state::<Cursor>() {
        Ok(state) => state,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    let all = &state.settings.click;
    let change = state
        .clicks
        .iter()
        .find(|c| c.is_for(&press.media, press.source));
    let look = change.map_or_else(|| all.clone(), |c| c.applied_to(all));
    let at = json!(time_arg(press.at));
    let reset = match change {
        Some(_) => vec![Step::of(CLICK_RESET, at_only(&at))],
        None => Vec::new(),
    };
    cx.groups.panel(ID);
    let group = Group {
        id: "one",
        title: names::CLICK_ONE,
        open: true,
        reset: Some(reset),
    };
    cx.group(ui, group, |ui, cx| {
        let mut edit = |field: &str, value: Value| {
            let mut fields = at_only(&at);
            fields.insert(field.to_owned(), value);
            cx.edits.push(vec![Step::of(CLICK_SET, fields)]);
        };
        style::look(ui, &look, &mut edit);
        style::timing(ui, &look, &mut edit);
        rows::note(ui, names::CLICK_ONE_NOTE);
    });
}

/// `--at <time>`.
fn at_only(at: &Value) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("at".to_owned(), at.clone());
    fields
}

fn button(button: PressButton) -> &'static str {
    match button {
        PressButton::Left => names::LEFT_BUTTON,
        PressButton::Right => names::RIGHT_BUTTON,
        PressButton::Middle => names::MIDDLE_BUTTON,
        PressButton::Back => names::BACK_BUTTON,
        PressButton::Forward => names::FORWARD_BUTTON,
    }
}
