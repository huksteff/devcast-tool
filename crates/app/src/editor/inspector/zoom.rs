//! «Зум» (mockup 3a): the camera motion of every zoom of the project — the rail tab, and the
//! shared part of a picked zoom's panel (`zoom_region.rs`). «Основное» — the preset of zoom-in and
//! zoom-out, joining close zooms, auto zooms after a recording; «Тонкая настройка» — smoothness,
//! zoom-in and zoom-out, motion blur. Every edit and «Сбросить» is `zoom motion` (the settings key
//! `zoom`); «Подобрать зумы по кликам» adds the zooms of `devcast zoom add --at clicks`.

use dc_fx_zoom::{
    MOTION_BLUR_MAX, Preset, TRANSITION_MAX, TRANSITION_MIN, ZoomEffect, ZoomSettings,
};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::HUE_ZOOM;
use dc_ui_kit::widgets::Button;
use eframe::egui::Ui;
use serde_json::{Map, Value, json};

use super::{Cx, Group, Header, args, header, padded, reset, rows};
use crate::editor::Action;

/// Id of the zoom panels in the remembered groups.
pub const ID: &str = "zoom";
/// The registry command of the camera motion.
const MOTION: &str = "zoom motion";

const PRESETS: [(Preset, &str); 2] = [
    (Preset::Focused, names::ZOOM_FOCUSED),
    (Preset::Smooth, names::ZOOM_SMOOTH),
];

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>) {
    let head = Header {
        hue: HUE_ZOOM,
        title: text::ZOOM,
        subtitle: names::WHOLE_PROJECT,
    };
    header(ui, &head, |_| {});
    let Some((now, default)) = settings(ui, cx) else {
        return;
    };
    let basic = Group {
        id: "basic",
        title: text::BASIC,
        open: true,
        reset: Some(motion_reset(
            &basic_args(&now, true),
            basic_args(&default, true),
        )),
    };
    cx.group(ui, basic, |ui, cx| {
        motion_rows(ui, cx, &now);
        if let Some(on) = rows::toggle(ui, names::ZOOM_AUTO, now.auto) {
            cx.actions.push(set("auto", json!(on)));
        }
    });
    fine(ui, cx, &now, &default);
    footer(ui, cx);
}

/// The motion settings of the project and of the defaults; `None` (and the error on the panel)
/// when the project's zoom state is broken.
pub fn settings(ui: &mut Ui, cx: &Cx<'_>) -> Option<(ZoomSettings, ZoomSettings)> {
    let read = cx
        .base
        .project
        .state::<ZoomEffect>()
        .and_then(|now| Ok((now.settings, cx.defaults_of::<ZoomEffect>()?.settings)));
    match read {
        Ok(both) => Some(both),
        Err(error) => {
            padded(ui, |ui| rows::note(ui, &error.to_string()));
            None
        }
    }
}

/// «Движение» (the preset; none chosen for durations of no preset) and «Склеить с соседним».
pub fn motion_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &ZoomSettings) {
    if let Some(preset) = rows::choice(ui, names::ZOOM_MOTION, now.preset(), &PRESETS) {
        cx.actions.push(set("preset", json!(preset)));
    }
    if let Some(on) = rows::toggle(ui, names::ZOOM_CONNECT, now.connect) {
        cx.actions.push(set("connect", json!(on)));
    }
}

/// «Тонкая настройка»: smoothness, zoom-in, zoom-out, motion blur.
pub fn fine(ui: &mut Ui, cx: &mut Cx<'_>, now: &ZoomSettings, default: &ZoomSettings) {
    let group = Group {
        id: "fine",
        title: text::FINE,
        open: false,
        reset: Some(motion_reset(&fine_args(now), fine_args(default))),
    };
    cx.group(ui, group, |ui, cx| {
        let unit = (0.01, "");
        if let Some(v) = rows::slider(ui, names::ZOOM_SMOOTHNESS, now.smoothness, 0.0..=1.0, unit) {
            cx.actions.push(set("smoothness", json!(v)));
        }
        let range = TRANSITION_MIN.as_secs_f64()..=TRANSITION_MAX.as_secs_f64();
        let secs = (0.01, names::SECONDS);
        let zoom_in = now.zoom_in.as_secs_f64();
        if let Some(v) = rows::slider(ui, names::ZOOM_IN, zoom_in, range.clone(), secs) {
            cx.actions.push(set("zoom_in", json!(v)));
        }
        let zoom_out = now.zoom_out.as_secs_f64();
        if let Some(v) = rows::slider(ui, names::ZOOM_OUT, zoom_out, range, secs) {
            cx.actions.push(set("zoom_out", json!(v)));
        }
        let blur = 0.0..=MOTION_BLUR_MAX;
        if let Some(v) = rows::slider(ui, names::ZOOM_BLUR, now.motion_blur, blur, (0.05, "")) {
            cx.actions.push(set("blur", json!(v)));
        }
    });
}

/// «Подобрать зумы по кликам» under the groups.
pub fn footer(ui: &mut Ui, cx: &mut Cx<'_>) {
    padded(ui, |ui| {
        let button = Button::new(names::ZOOM_SUGGEST).small().icon(Icon::Wand);
        let size = [ui.available_width(), button.size(ui).y];
        if ui.add_sized(size, button).clicked() {
            cx.actions.push(Action::SuggestZooms);
        }
    });
}

/// `zoom motion --<field> <value>`.
fn set(field: &'static str, value: Value) -> Action {
    Action::op(MOTION, [(field, value)])
}

/// `zoom motion` with the default fields of a group, as steps of «Сбросить».
pub fn motion_reset(now: &Map<String, Value>, default: Map<String, Value>) -> Vec<dc_ops::Step> {
    reset(MOTION, now, default).into_iter().collect()
}

/// The motion fields of «Основное» as arguments of `zoom motion`: the durations the preset
/// sets, joining, and auto zooms when the panel shows them (`auto`).
pub fn basic_args(s: &ZoomSettings, auto: bool) -> Map<String, Value> {
    let mut fields = args([
        ("zoom_in", json!(s.zoom_in.as_secs_f64())),
        ("zoom_out", json!(s.zoom_out.as_secs_f64())),
        ("connect", json!(s.connect)),
    ]);
    if auto {
        fields.insert("auto".to_owned(), json!(s.auto));
    }
    fields
}

/// The fields of «Тонкая настройка» as arguments of `zoom motion`.
fn fine_args(s: &ZoomSettings) -> Map<String, Value> {
    args([
        ("smoothness", json!(s.smoothness)),
        ("zoom_in", json!(s.zoom_in.as_secs_f64())),
        ("zoom_out", json!(s.zoom_out.as_secs_f64())),
        ("blur", json!(s.motion_blur)),
    ])
}
