//! «Курсор и клики» (mockup 3a): the cursor and the click effect of every recording — the rail
//! tab «Курсор» ([`ui`]). «Курсор» — show, the set, size, motion (`cursor`); the set is
//! «Системный» only: own cursor pictures are frozen by the owner (QUESTIONS.md, 2026-10-29), so
//! «Своя картинка» is off with that reason. «Эффект клика» — style, colour, size (`clicks`);
//! «Тонкая настройка» — opacity and duration of clicks (`clicks`), bounce, its length, sway,
//! motion blur, loop (`cursor`). A single click is edited from its dot on the «Курсор» row
//! ([`click`], `panel_click.rs`, with the facts [`Press`]). Edits go to `cx.edits`.

use dc_core::{Effect, Step};
use dc_ui_kit::egui::Ui;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{Cx, Group, Header, args, header, padded, reset, rows};
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::HUE_CURSOR;
use dc_ui_kit::widgets::{Button, dropdown};
use serde_json::{Map, Value, json};

pub use crate::panel_click::{Press, PressButton, click};
use crate::panel_style as style;
use crate::{
    BLUR_MAX, BOUNCE_MAX, BOUNCE_TIME_MAX, BOUNCE_TIME_MIN, Cursor, CursorSettings, SIZE_MAX,
    SIZE_MIN, SWAY_MAX, Smoothing,
};

/// Id of the cursor panels in the remembered groups.
pub const ID: &str = Cursor::ID;
/// The registry commands of the cursor and of every click.
const CURSOR: &str = "cursor";
const CLICKS: &str = "clicks";

const MOTIONS: [(Smoothing, &str); 3] = [
    (Smoothing::Exact, names::CURSOR_EXACT),
    (Smoothing::Focused, names::ZOOM_FOCUSED),
    (Smoothing::Smooth, names::ZOOM_SMOOTH),
];

/// The «Курсор» tab.
pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>) {
    let head = Header {
        hue: HUE_CURSOR,
        title: names::CURSOR_PANEL,
        subtitle: names::WHOLE_PROJECT,
    };
    header(ui, &head, |_| {});
    let Some((now, default)) = settings(ui, cx) else {
        return;
    };
    let group = Group {
        id: "cursor",
        title: names::CURSOR_GROUP,
        open: true,
        reset: Some(steps(CURSOR, &cursor_args(&now), cursor_args(&default))),
    };
    cx.group(ui, group, |ui, cx| cursor_rows(ui, cx, &now));
    let look = style::look_args(&now.click);
    let group = Group {
        id: "click",
        title: names::CLICK_GROUP,
        open: true,
        reset: Some(steps(CLICKS, &look, style::look_args(&default.click))),
    };
    cx.group(ui, group, |ui, cx| {
        style::look(ui, &now.click, |field, value| {
            cx.edits.push(set(CLICKS, field, value));
        });
    });
    fine(ui, cx, &now, &default);
}

/// The cursor settings of the project and of the defaults; `None` (and the error on the panel)
/// when the project's cursor state is broken.
fn settings(ui: &mut Ui, cx: &Cx<'_>) -> Option<(CursorSettings, CursorSettings)> {
    let read = cx
        .project
        .state::<Cursor>()
        .and_then(|now| Ok((now.settings, cx.defaults_of::<Cursor>()?.settings)));
    match read {
        Ok(both) => Some(both),
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            None
        }
    }
}

/// Show, the set and the own picture (off), size, motion.
fn cursor_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &CursorSettings) {
    if let Some(on) = rows::toggle(ui, names::CURSOR_SHOW, now.show) {
        cx.edits.push(set(CURSOR, "show", json!(on)));
    }
    rows::row(ui, names::CURSOR_SET, |ui| {
        let width = ui.available_width();
        let sets = [((), names::CURSOR_SYSTEM)];
        dropdown(ui, names::CURSOR_SET, &mut (), &sets, width)
    });
    rows::row(ui, names::CURSOR_OWN, |ui| {
        let button = Button::new(names::CURSOR_OWN_PICK)
            .small()
            .icon(Icon::Folder);
        ui.add_enabled(false, button)
            .on_disabled_hover_text(names::CURSOR_OWN_FROZEN)
    });
    let unit = (0.1, names::TIMES);
    let range = SIZE_MIN..=SIZE_MAX;
    if let Some(v) = rows::slider(ui, names::CURSOR_SIZE, now.size, range, unit) {
        cx.edits.push(set(CURSOR, "size", json!(v)));
    }
    let motion = Some(now.smoothing);
    if let Some(smoothing) = rows::choice_under(ui, names::CURSOR_MOTION, motion, &MOTIONS) {
        cx.edits.push(set(CURSOR, "smoothing", json!(smoothing)));
    }
}

/// «Тонкая настройка»: the timing of clicks, then the motion of the cursor.
fn fine(ui: &mut Ui, cx: &mut Cx<'_>, now: &CursorSettings, default: &CursorSettings) {
    let timing = style::timing_args(&now.click);
    let mut reset = steps(CLICKS, &timing, style::timing_args(&default.click));
    reset.extend(steps(CURSOR, &motion_args(now), motion_args(default)));
    let group = Group {
        id: "fine",
        title: text::FINE,
        open: false,
        reset: Some(reset),
    };
    cx.group(ui, group, |ui, cx| {
        style::timing(ui, &now.click, |field, value| {
            cx.edits.push(set(CLICKS, field, value));
        });
        let tenth = (0.1, "");
        let mut slider = |label, value, max, field| {
            if let Some(v) = rows::slider(ui, label, value, 0.0..=max, tenth) {
                cx.edits.push(set(CURSOR, field, json!(v)));
            }
        };
        slider(names::CURSOR_BOUNCE, now.bounce, BOUNCE_MAX, "bounce");
        let range = BOUNCE_TIME_MIN.as_secs_f64()..=BOUNCE_TIME_MAX.as_secs_f64();
        let length = now.bounce_time.as_secs_f64();
        let secs = (0.01, names::SECONDS);
        if let Some(v) = rows::slider(ui, names::CURSOR_BOUNCE_TIME, length, range, secs) {
            cx.edits.push(set(CURSOR, "bounce_time", json!(v)));
        }
        let mut slider = |label, value, max, field| {
            if let Some(v) = rows::slider(ui, label, value, 0.0..=max, tenth) {
                cx.edits.push(set(CURSOR, field, json!(v)));
            }
        };
        slider(names::CURSOR_SWAY, now.sway, SWAY_MAX, "sway");
        slider(names::CURSOR_BLUR, now.blur, BLUR_MAX, "blur");
        let hint = names::CURSOR_LOOP_NOTE;
        if let Some(on) = rows::toggle_hint(ui, names::CURSOR_LOOP, now.looped, hint) {
            cx.edits.push(set(CURSOR, "loop", json!(on)));
        }
    });
}

/// `<op> --<field> <value>`.
fn set(op: &str, field: &str, value: Value) -> Vec<Step> {
    let mut fields = Map::new();
    fields.insert(field.to_owned(), value);
    vec![Step::new(op, fields)]
}

/// `op` with the default fields of a group, as the steps of «Сбросить».
fn steps(op: &str, now: &Map<String, Value>, default: Map<String, Value>) -> Vec<Step> {
    reset(op, now, default).into_iter().collect()
}

/// The fields of the «Курсор» group as arguments of `cursor`.
fn cursor_args(s: &CursorSettings) -> Map<String, Value> {
    args([
        ("show", json!(s.show)),
        ("size", json!(s.size)),
        ("smoothing", json!(s.smoothing)),
    ])
}

/// The cursor fields of «Тонкая настройка» as arguments of `cursor`.
fn motion_args(s: &CursorSettings) -> Map<String, Value> {
    args([
        ("bounce", json!(s.bounce)),
        ("bounce_time", json!(s.bounce_time.as_secs_f64())),
        ("sway", json!(s.sway)),
        ("blur", json!(s.blur)),
        ("loop", json!(s.looped)),
    ])
}
