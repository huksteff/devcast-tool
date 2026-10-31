//! The panel of one click picked on the «Курсор» row (mockup 3a «Клик»): the head says when and
//! which button; «Эффект этого клика» shows the click's effect — its own fields over the effect
//! of every click — and each edit is `click set --at <output time>` (the change of this press,
//! `ClickChange`); «Сбросить» is `click reset`: the click is like the others again. The
//! «Выкл» tile hides the effect of this click. A press is the recording's: it cannot be removed.

use dc_capture::input::Button;
use dc_fx_cursor::Cursor;
use dc_ops::Step;
use dc_ui_kit::strings::inspector as names;
use dc_ui_kit::tokens::HUE_CURSOR;
use eframe::egui::Ui;
use serde_json::{Map, Value, json};

use super::{Cx, Group, Header, click_style, cursor, header, padded, rows};
use crate::editor::montage;
use crate::editor::timeline::{Dot, timecode};

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, dot: &Dot) {
    let subtitle = names::click_about(&timecode(dot.at), button(dot.button), dot.double);
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
        .find(|c| c.is_for(&dot.media, dot.source));
    let style = change.map_or_else(|| all.clone(), |c| c.applied_to(all));
    let at = json!(montage::arg(dot.at));
    let reset = match change {
        Some(_) => vec![Step::new("click reset", at_only(&at))],
        None => Vec::new(),
    };
    cx.panel = cursor::ID;
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
            cx.actions.push(crate::editor::Action::Apply(vec![Step::new(
                "click set",
                fields,
            )]));
        };
        click_style::look(ui, &style, &mut edit);
        click_style::timing(ui, &style, &mut edit);
        rows::note(ui, names::CLICK_ONE_NOTE);
    });
}

/// `--at <time>`.
fn at_only(at: &Value) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("at".to_owned(), at.clone());
    fields
}

fn button(button: Button) -> &'static str {
    match button {
        Button::Left => names::LEFT_BUTTON,
        Button::Right => names::RIGHT_BUTTON,
        Button::Middle => names::MIDDLE_BUTTON,
        Button::X1 => names::BACK_BUTTON,
        Button::X2 => names::FORWARD_BUTTON,
    }
}
