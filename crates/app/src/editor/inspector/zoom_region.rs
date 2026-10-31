//! The panel of a picked zoom (mockup 3a): «Основное» — mode, scale and focus point of this zoom
//! (`zoom set`), then the motion shared by every zoom (`zoom.rs`: `zoom motion`); «Тонкая
//! настройка» and «Подобрать зумы по кликам» as on the rail tab. «Сбросить» of «Основное» brings
//! the zoom to what `zoom add` gives (scale 1.5, follow, focus in the centre) and the shared
//! fields to the defaults, one undo step. The frame on the preview
//! (`editor/stage/zoom.rs`) edits the same scale and focus.

use dc_fx_zoom::{
    DEFAULT_SCALE, Mode, SCALE_MAX, SCALE_MIN, Zoom, ZoomEffect, ZoomId, ZoomSettings,
};
use dc_ops::Step;
use dc_ui_kit::strings::{editor as text, inspector as names};
use eframe::egui::Ui;
use serde_json::{Map, Value, json};

use super::{Cx, Group, args, padded, reset, rows, zoom};
use crate::editor::Action;

const SET: &str = "zoom set";

const MODES: [(Mode, &str); 2] = [
    (Mode::Follow, names::ZOOM_FOLLOW),
    (Mode::Fixed, names::ZOOM_FIXED),
];

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, id: ZoomId) {
    let Some((now, default)) = zoom::settings(ui, cx) else {
        return;
    };
    // The selection keeps only what the rows show, so the zoom is there.
    let Ok(state) = cx.project.state::<ZoomEffect>() else {
        return;
    };
    let Some(region) = state.zooms.into_iter().find(|z| z.id == id) else {
        return;
    };
    cx.panel = zoom::ID;
    let basic = Group {
        id: "region",
        title: text::BASIC,
        open: true,
        reset: Some(basic_reset(&region, &now, &default)),
    };
    cx.group(ui, basic, |ui, cx| {
        region_rows(ui, cx, &region);
        zoom::motion_rows(ui, cx, &now);
    });
    zoom::fine(ui, cx, &now, &default);
    padded(ui, |ui| rows::note(ui, names::ZOOM_SHARED));
    zoom::footer(ui, cx);
}

/// Mode with what it does, scale, focus point in percent of the frame.
fn region_rows(ui: &mut Ui, cx: &mut Cx<'_>, region: &Zoom) {
    let id = region.id;
    if let Some(mode) = rows::choice(ui, names::ZOOM_MODE, Some(region.mode), &MODES) {
        cx.actions.push(set(id, "mode", json!(mode)));
    }
    let note = match region.mode {
        Mode::Follow => names::ZOOM_FOLLOW_NOTE,
        Mode::Fixed => names::ZOOM_FIXED_NOTE,
    };
    rows::note(ui, note);
    let scale = (0.05, names::TIMES);
    let range = SCALE_MIN..=SCALE_MAX;
    if let Some(v) = rows::slider(ui, names::ZOOM_SCALE, region.scale, range, scale) {
        cx.actions.push(set(id, "scale", json!(v)));
    }
    let percent = (1.0, names::PERCENT);
    for (label, axis, value) in [
        (names::ZOOM_X, "x", region.focus.x),
        (names::ZOOM_Y, "y", region.focus.y),
    ] {
        if let Some(v) = rows::slider(ui, label, value * 100.0, 0.0..=100.0, percent) {
            cx.actions.push(set(id, axis, json!(v / 100.0)));
        }
    }
}

/// `zoom set <id> --<field> <value>`.
fn set(id: ZoomId, field: &'static str, value: Value) -> Action {
    Action::op(SET, [("id", json!(id)), (field, value)])
}

/// The fields of the zoom in «Основное» as arguments of `zoom set`.
fn region_args(id: ZoomId, scale: f64, mode: Mode, (x, y): (f64, f64)) -> Map<String, Value> {
    args([
        ("id", json!(id)),
        ("scale", json!(scale)),
        ("mode", json!(mode)),
        ("x", json!(x)),
        ("y", json!(y)),
    ])
}

fn basic_reset(region: &Zoom, now: &ZoomSettings, default: &ZoomSettings) -> Vec<Step> {
    let focus = (region.focus.x, region.focus.y);
    let own = region_args(region.id, region.scale, region.mode, focus);
    let added = region_args(region.id, DEFAULT_SCALE, Mode::default(), (0.5, 0.5));
    let mut steps: Vec<Step> = reset(SET, &own, added).into_iter().collect();
    steps.extend(zoom::motion_reset(
        &zoom::basic_args(now, false),
        zoom::basic_args(default, false),
    ));
    steps
}
