//! The look of every caption of the project (mockup 3b «Субтитры · стиль»): «Стиль» — font,
//! size, word colours, shadow, animation; «Тонкая настройка» — lines, place, width and the plate.
//! Every edit and «Сбросить» is `captions style` (the settings key `captions`).

use dc_core::OpName;
use dc_core::Step;
use dc_ui_kit::egui::Ui;
use dc_ui_kit::panel::{Cx, Group, args, reset, rows};
use dc_ui_kit::strings::{captions as text, editor, inspector as names};
use serde_json::{Map, Value, json};

use crate::{
    Animation, BOTTOM_MAX, CaptionsSettings, RADIUS_MAX, ROWS_MAX, SIZE_MAX, SIZE_MIN, WIDTH_MAX,
    WIDTH_MIN,
};

/// The command of the look.
pub const OP: OpName = OpName::of::<crate::CaptionsStyle>();

const ANIMATIONS: [(Animation, &str); 4] = [
    (Animation::None, text::NO_ANIMATION),
    (Animation::Fade, text::FADE),
    (Animation::Rise, text::RISE),
    (Animation::Pop, text::POP),
];

/// «Стиль» and «Тонкая настройка» of the look `now`, «Сбросить» to `default`.
pub fn groups(ui: &mut Ui, cx: &mut Cx<'_>, now: &CaptionsSettings, default: &CaptionsSettings) {
    let style = Group {
        id: "style",
        title: text::STYLE,
        open: false,
        reset: Some(
            reset(OP, &style_args(now), style_args(default))
                .into_iter()
                .collect(),
        ),
    };
    cx.group(ui, style, |ui, cx| style_rows(ui, cx, now));
    let fine = Group {
        id: "fine",
        title: editor::FINE,
        open: false,
        reset: Some(
            reset(OP, &fine_args(now), fine_args(default))
                .into_iter()
                .collect(),
        ),
    };
    cx.group(ui, fine, |ui, cx| fine_rows(ui, cx, now));
}

fn style_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &CaptionsSettings) {
    let choose = Step::of(OP, Map::new());
    if let Some(font) = cx.font(ui, text::FONT, &now.font, choose) {
        cx.apply(OP, [("font", json!(font))]);
    }
    let size = (1.0, text::PX);
    if let Some(v) = rows::slider(ui, text::SIZE, now.size, SIZE_MIN..=SIZE_MAX, size) {
        cx.apply(OP, [("size", json!(v))]);
    }
    let colors = [
        (text::COLOR, now.color),
        (text::HIGHLIGHT, now.highlight),
        (text::INACTIVE, now.inactive),
    ];
    if let Some((i, color)) = rows::colors(ui, text::WORDS, &colors) {
        let field = ["color", "highlight", "inactive"][i];
        cx.apply(OP, [(field, json!(color))]);
    }
    if let Some(on) = rows::toggle(ui, text::SHADOW, now.shadow) {
        cx.apply(OP, [("shadow", json!(on))]);
    }
    let animation = Some(now.animation);
    if let Some(a) = rows::choice_under(ui, text::ANIMATION, animation, &ANIMATIONS) {
        cx.apply(OP, [("animation", json!(a))]);
    }
}

fn fine_rows(ui: &mut Ui, cx: &mut Cx<'_>, now: &CaptionsSettings) {
    let lines: Vec<(u32, String)> = (1..=ROWS_MAX).map(|n| (n, n.to_string())).collect();
    let options: Vec<(u32, &str)> = lines.iter().map(|(n, s)| (*n, s.as_str())).collect();
    if let Some(n) = rows::choice(ui, text::ROWS, Some(now.rows), &options) {
        cx.apply(OP, [("rows", json!(n))]);
    }
    let percent = (0.5, names::PERCENT);
    if let Some(v) = rows::slider(ui, text::BOTTOM, now.bottom, 0.0..=BOTTOM_MAX, percent) {
        cx.apply(OP, [("bottom", json!(v))]);
    }
    let width = (1.0, names::PERCENT);
    if let Some(v) = rows::slider(ui, text::WIDTH, now.width, WIDTH_MIN..=WIDTH_MAX, width) {
        cx.apply(OP, [("width", json!(v))]);
    }
    // The plate's opacity is 0..1 in the project and percent on the panel.
    let plate = now.background * 100.0;
    if let Some(v) = rows::slider(ui, text::PLATE, plate, 0.0..=100.0, (1.0, names::PERCENT)) {
        cx.apply(OP, [("background", json!(v / 100.0))]);
    }
    let radius = (0.5, text::PX);
    if let Some(v) = rows::slider(ui, text::RADIUS, now.radius, 0.0..=RADIUS_MAX, radius) {
        cx.apply(OP, [("radius", json!(v))]);
    }
}

/// The fields of «Стиль» as arguments of `captions style`.
fn style_args(s: &CaptionsSettings) -> Map<String, Value> {
    args([
        ("font", json!(s.font)),
        ("size", json!(s.size)),
        ("color", json!(s.color)),
        ("highlight", json!(s.highlight)),
        ("inactive", json!(s.inactive)),
        ("shadow", json!(s.shadow)),
        ("animation", json!(s.animation)),
    ])
}

/// The fields of «Тонкая настройка» as arguments of `captions style`.
fn fine_args(s: &CaptionsSettings) -> Map<String, Value> {
    args([
        ("rows", json!(s.rows)),
        ("bottom", json!(s.bottom)),
        ("width", json!(s.width)),
        ("background", json!(s.background)),
        ("radius", json!(s.radius)),
    ])
}
