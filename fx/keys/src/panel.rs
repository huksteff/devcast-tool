//! «Показ клавиш» (mockup 3b): the inspector panel of the plate settings for the whole project.
//! The subtitle counts the shortcuts of the recordings; «Основное» — show, place, size and a
//! sample plate in the project's colours from its most repeated shortcut ([`Shortcuts`]);
//! «Тонкая настройка» — colours and how long a shortcut stays.
//! Every edit and «Сбросить» is the `keys` command (`devcast keys …`, the settings key `keys`).

use std::collections::BTreeSet;

use dc_core::{Effect, KeyTracks};
use dc_render::Rgba;
use dc_ui_kit::egui::{self, Color32, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::panel::{Cx, Group, Header, args, header, padded, reset, rows};
use dc_ui_kit::strings::{editor as text, inspector as names};
use dc_ui_kit::tokens::{BORDER, FS_L, HUE_KEYS};
use serde_json::{Map, Value, json};

use crate::{
    DURATION_MAX, DURATION_MIN, Keys, KeysSettings, Position, Prepared, SIZE_MAX, SIZE_MIN,
};

/// Padding of the sample plate around its letters and its corners, points (mockup: 12 × 6, 6).
const PLATE_PAD: egui::Vec2 = vec2(12.0, 6.0);
const PLATE_RADIUS: u8 = 6;

/// Id of the panel in the remembered groups.
pub const ID: &str = Keys::ID;
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

/// What the recordings hold for the panel: the editor keeps it with the «Клавиши» row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shortcuts {
    /// Different shortcuts pressed in the recordings, cut parts too.
    pub names: usize,
    /// The plate pressed most often on the output (the first of equals): the sample of the panel.
    pub sample: Option<Sample>,
}

/// A plate of the project: its shortcut and how many presses it counts.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub name: String,
    pub count: usize,
}

impl Shortcuts {
    /// The facts of the shortcuts `keys` of the recordings placed as `prepared` shows them.
    pub fn of(prepared: &Prepared, keys: &KeyTracks) -> Self {
        let names: BTreeSet<&str> = keys.values().flatten().map(|k| k.name.as_str()).collect();
        let mut sample: Option<Sample> = None;
        for shown in prepared.shown() {
            let count = shown.presses.len();
            if count > 0 && sample.as_ref().is_none_or(|s| count > s.count) {
                sample = Some(Sample {
                    name: shown.name.clone(),
                    count,
                });
            }
        }
        Self {
            names: names.len(),
            sample,
        }
    }
}

/// The panel: its edits go to `cx.edits`.
pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, shortcuts: &Shortcuts) {
    cx.groups.panel(ID);
    let found = names::shortcuts_found(shortcuts.names);
    let head = Header {
        hue: HUE_KEYS,
        title: text::KEYS,
        subtitle: &found,
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
        reset: Some(
            reset(OP, &basic_args(&now), basic_args(&default))
                .into_iter()
                .collect(),
        ),
    };
    cx.group(ui, basic, |ui, cx| basic_rows(ui, cx, &now, shortcuts));
    let fine = Group {
        id: "fine",
        title: text::FINE,
        open: false,
        reset: Some(
            reset(OP, &fine_args(&now), fine_args(&default))
                .into_iter()
                .collect(),
        ),
    };
    cx.group(ui, fine, |ui, cx| fine_rows(ui, cx, &now));
    padded(ui, |ui| rows::note(ui, names::KEYS_NOTE));
}

fn basic_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &KeysSettings, shortcuts: &Shortcuts) {
    if let Some(show) = rows::toggle(ui, names::KEYS_SHOW, now.show) {
        cx.apply(OP, [("show", json!(show))]);
    }
    let places: Vec<&str> = PLACES.iter().map(|(_, name)| *name).collect();
    let place = PLACES
        .iter()
        .position(|(p, _)| *p == now.position)
        .unwrap_or_default();
    if let Some(i) = rows::anchors(ui, names::KEYS_WHERE, place, &places) {
        cx.apply(OP, [("position", json!(PLACES[i].0))]);
    }
    let size = (SIZE_MIN..=SIZE_MAX, (0.05, names::TIMES));
    if let Some(v) = rows::slider(ui, names::KEYS_SIZE, now.size, size.0, size.1) {
        cx.apply(OP, [("size", json!(v))]);
    }
    if let Some(sample) = &shortcuts.sample {
        plate(ui, sample, now);
    }
}

/// «Ctrl + Shift + P ×2» on a plate of the project's colours, in the middle of the row.
fn plate(ui: &mut Ui, sample: &Sample, now: &KeysSettings) {
    let line = names::plate_sample(&sample.name, sample.count);
    let letters = color32(now.color);
    let galley =
        ui.painter()
            .layout_no_wrap(line.clone(), fonts::text(FS_L, Weight::Semibold), letters);
    let size = galley.size() + 2.0 * PLATE_PAD;
    ui.vertical_centered(|ui| {
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, &line));
        // The edge keeps a see-through plate in sight on the panel (mockup: a framed plate).
        let edge = Stroke::new(1.0, BORDER);
        let fill = color32(now.background);
        ui.painter()
            .rect(rect, PLATE_RADIUS, fill, edge, StrokeKind::Inside);
        ui.painter().galley(rect.min + PLATE_PAD, galley, letters);
    });
}

fn color32(Rgba([r, g, b, a]): Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn fine_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &KeysSettings) {
    let colors = [
        (names::KEYS_LETTERS, now.color),
        (names::KEYS_PLATE, now.background),
    ];
    if let Some((i, color)) = rows::colors(ui, names::KEYS_COLORS, &colors) {
        let field = if i == 0 { "color" } else { "background" };
        cx.apply(OP, [(field, json!(color))]);
    }
    let range = DURATION_MIN.as_secs_f64()..=DURATION_MAX.as_secs_f64();
    let hold = now.duration.as_secs_f64();
    if let Some(v) = rows::slider(ui, names::KEYS_HOLD, hold, range, (0.1, names::SECONDS)) {
        cx.apply(OP, [("duration", json!(v))]);
    }
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
