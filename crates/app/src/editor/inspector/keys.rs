//! «Показ клавиш» (mockup 3b): the settings of the pressed-keys plate for the whole project.
//! «Основное» — show, place, size; «Тонкая настройка» — colours and how long a shortcut stays.
//! Every edit and «Сбросить» is the `keys` command (`devcast keys …`, the settings key `keys`).

use dc_fx_keys::{Keys, KeysSettings, Position};
use dc_ops::Step;
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::HUE_KEYS;
use eframe::egui::Ui;
use serde_json::{Map, Value, json};

use super::{Cx, Group, Header, header, padded, rows};
use crate::editor::Action;

/// Id of the panel in the remembered groups.
pub const ID: &str = "keys";
/// The registry command of the plate settings.
const OP: &str = "keys";

/// The places of the anchor grid, in its order (three in a row).
const PLACES: [(Position, &str); 6] = [
    (Position::TopLeft, names::TOP_LEFT),
    (Position::Top, names::TOP),
    (Position::TopRight, names::TOP_RIGHT),
    (Position::BottomLeft, names::BOTTOM_LEFT),
    (Position::Bottom, names::BOTTOM),
    (Position::BottomRight, names::BOTTOM_RIGHT),
];

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>) {
    let head = Header {
        hue: HUE_KEYS,
        title: text::KEYS,
        subtitle: names::WHOLE_PROJECT,
    };
    header(ui, &head, |_| {});
    let read = cx
        .project
        .state::<Keys>()
        .and_then(|now| Ok((now.settings, cx.defaults_of::<Keys>()?.settings)));
    let (now, default) = match read {
        Ok(both) => both,
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            return;
        }
    };
    let basic = Group {
        id: "basic",
        title: text::BASIC,
        open: true,
        reset: Some(reset(&basic_args(&now), basic_args(&default))),
    };
    cx.group(ui, basic, |ui, cx| basic_rows(ui, cx, &now));
    let fine = Group {
        id: "fine",
        title: text::FINE,
        open: false,
        reset: Some(reset(&fine_args(&now), fine_args(&default))),
    };
    cx.group(ui, fine, |ui, cx| fine_rows(ui, cx, &now));
    padded(ui, |ui| rows::note(ui, names::KEYS_NOTE));
}

fn basic_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &KeysSettings) {
    if let Some(show) = rows::toggle(ui, names::KEYS_SHOW, now.show) {
        cx.actions.push(set("show", json!(show)));
    }
    let places: Vec<&str> = PLACES.iter().map(|(_, name)| *name).collect();
    let place = PLACES
        .iter()
        .position(|(p, _)| *p == now.position)
        .unwrap_or_default();
    if let Some(i) = rows::anchors(ui, names::KEYS_WHERE, place, &places) {
        cx.actions.push(set("position", json!(PLACES[i].0)));
    }
    let size = (
        dc_fx_keys::SIZE_MIN..=dc_fx_keys::SIZE_MAX,
        (0.05, names::TIMES),
    );
    if let Some(v) = rows::slider(ui, names::KEYS_SIZE, now.size, size.0, size.1) {
        cx.actions.push(set("size", json!(v)));
    }
}

fn fine_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &KeysSettings) {
    let colors = [
        (names::KEYS_LETTERS, now.color),
        (names::KEYS_PLATE, now.background),
    ];
    if let Some((i, color)) = rows::colors(ui, names::KEYS_COLORS, &colors) {
        let field = if i == 0 { "color" } else { "background" };
        cx.actions.push(set(field, json!(color)));
    }
    let range = dc_fx_keys::DURATION_MIN.as_secs_f64()..=dc_fx_keys::DURATION_MAX.as_secs_f64();
    let hold = now.duration.as_secs_f64();
    if let Some(v) = rows::slider(ui, names::KEYS_HOLD, hold, range, (0.1, names::SECONDS)) {
        cx.actions.push(set("duration", json!(v)));
    }
}

/// `keys --<field> <value>`.
fn set(field: &'static str, value: Value) -> Action {
    Action::op(OP, [(field, value)])
}

/// The fields of «Основное» as arguments of `keys`.
fn basic_args(s: &KeysSettings) -> Map<String, Value> {
    args([
        ("show", json!(s.show)),
        ("position", json!(s.position)),
        ("size", json!(s.size)),
    ])
}

/// The fields of «Тонкая настройка» as arguments of `keys`.
fn fine_args(s: &KeysSettings) -> Map<String, Value> {
    args([
        ("color", json!(s.color)),
        ("background", json!(s.background)),
        ("duration", json!(s.duration.as_secs_f64())),
    ])
}

fn args<const N: usize>(fields: [(&str, Value); N]) -> Map<String, Value> {
    fields
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// `keys` with the default fields of a group; nothing when they are the defaults already.
fn reset(now: &Map<String, Value>, default: Map<String, Value>) -> Vec<Step> {
    if *now == default {
        return Vec::new();
    }
    vec![Step::new(OP, default)]
}
