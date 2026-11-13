//! «Зум» (mockup 3a): the camera motion of every zoom of the project — the rail tab ([`ui`]), and
//! the shared part of a picked zoom's panel ([`region`], `panel_region.rs`). «Основное» — the
//! preset of zoom-in and zoom-out, joining close zooms, auto zooms after a recording; «Тонкая
//! настройка» — smoothness, zoom-in and zoom-out, motion blur. Every edit and «Сбросить» is
//! `zoom motion` (the settings key `zoom`) in `cx.edits`; «Подобрать зумы по кликам» is the step
//! `zoom add --at clicks` ([`by_clicks`]) in `cx.adds`, so the editor picks the added zooms.
//! The panels need no facts of the editor: the zooms and their motion are in the project.

use dc_core::OpName;
use dc_core::{Effect, Step};
use dc_ui_kit::egui::Ui;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{Cx, Group, Header, args, header, padded, reset, rows};
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::HUE_ZOOM;
use dc_ui_kit::widgets::Button;
use serde_json::{Map, Value, json};

pub use crate::panel_region::region;
use crate::{
    MOTION_BLUR_MAX, Preset, TRANSITION_MAX, TRANSITION_MIN, ZoomEffect, ZoomSettings, by_clicks,
};

/// Id of the zoom panels in the remembered groups.
pub const ID: &str = ZoomEffect::ID;
/// The registry command of the camera motion.
const MOTION: OpName = OpName::of::<crate::ZoomMotion>();

const PRESETS: [(Preset, &str); 2] = [
    (Preset::Focused, names::ZOOM_FOCUSED),
    (Preset::Smooth, names::ZOOM_SMOOTH),
];

/// The «Зум» tab.
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
            cx.edits.push(set("auto", json!(on)));
        }
    });
    fine(ui, cx, &now, &default);
    footer(ui, cx);
}

/// The motion settings of the project and of the defaults; `None` (and the error on the panel)
/// when the project's zoom state is broken.
pub(crate) fn settings(ui: &mut Ui, cx: &Cx<'_>) -> Option<(ZoomSettings, ZoomSettings)> {
    let read = cx
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
pub(crate) fn motion_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &ZoomSettings) {
    if let Some(preset) = rows::choice(ui, names::ZOOM_MOTION, now.preset(), &PRESETS) {
        cx.edits.push(set("preset", json!(preset)));
    }
    if let Some(on) = rows::toggle(ui, names::ZOOM_CONNECT, now.connect) {
        cx.edits.push(set("connect", json!(on)));
    }
}

/// «Дополнительно»: smoothness, zoom-in, zoom-out, motion blur.
pub(crate) fn fine(ui: &mut Ui, cx: &mut Cx<'_>, now: &ZoomSettings, default: &ZoomSettings) {
    let group = Group {
        id: "fine",
        title: text::FINE,
        open: false,
        reset: Some(motion_reset(&fine_args(now), fine_args(default))),
    };
    cx.group(ui, group, |ui, cx| {
        let unit = (0.01, "");
        if let Some(v) = rows::slider(ui, names::ZOOM_SMOOTHNESS, now.smoothness, 0.0..=1.0, unit) {
            cx.edits.push(set("smoothness", json!(v)));
        }
        let range = TRANSITION_MIN.as_secs_f64()..=TRANSITION_MAX.as_secs_f64();
        let secs = (0.01, names::SECONDS);
        let zoom_in = now.zoom_in.as_secs_f64();
        if let Some(v) = rows::slider(ui, names::ZOOM_IN, zoom_in, range.clone(), secs) {
            cx.edits.push(set("zoom_in", json!(v)));
        }
        let zoom_out = now.zoom_out.as_secs_f64();
        if let Some(v) = rows::slider(ui, names::ZOOM_OUT, zoom_out, range, secs) {
            cx.edits.push(set("zoom_out", json!(v)));
        }
        let blur = 0.0..=MOTION_BLUR_MAX;
        if let Some(v) = rows::slider(ui, names::ZOOM_BLUR, now.motion_blur, blur, (0.05, "")) {
            cx.edits.push(set("blur", json!(v)));
        }
    });
}

/// «Подобрать зумы по кликам» under the groups.
pub(crate) fn footer(ui: &mut Ui, cx: &mut Cx<'_>) {
    padded(ui, |ui| {
        let button = Button::new(names::ZOOM_SUGGEST).small().icon(Icon::Wand);
        let size = [ui.available_width(), button.size(ui).y];
        if ui.add_sized(size, button).clicked() {
            cx.adds.push(vec![by_clicks()]);
        }
    });
}

/// `zoom motion --<field> <value>`.
fn set(field: &'static str, value: Value) -> Vec<Step> {
    vec![Step::of(MOTION, args([(field, value)]))]
}

/// `zoom motion` with the default fields of a group, as steps of «Сбросить».
pub(crate) fn motion_reset(now: &Map<String, Value>, default: Map<String, Value>) -> Vec<Step> {
    reset(MOTION, now, default).into_iter().collect()
}

/// The motion fields of «Основное» as arguments of `zoom motion`: the durations the preset
/// sets, joining, and auto zooms when the panel shows them (`auto`).
pub(crate) fn basic_args(s: &ZoomSettings, auto: bool) -> Map<String, Value> {
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

/// The fields of «Дополнительно» as arguments of `zoom motion`.
fn fine_args(s: &ZoomSettings) -> Map<String, Value> {
    args([
        ("smoothness", json!(s.smoothness)),
        ("zoom_in", json!(s.zoom_in.as_secs_f64())),
        ("zoom_out", json!(s.zoom_out.as_secs_f64())),
        ("blur", json!(s.motion_blur)),
    ])
}
