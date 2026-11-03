//! Rows of a click effect (mockup 3a «Эффект клика»), shared by the «Курсор» tab (`clicks`, every
//! click) and the panel of one click (`click set --at`): the look — tiles of the five styles,
//! the colour (the first five of the v1 palette and an own colour), the size — and the timing —
//! opacity and duration. Both commands take the same fields.

use dc_render::{BLUE, Rgba};
use dc_ui_kit::egui::{Color32, Painter, Pos2, Rect, Stroke, Ui, vec2};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{args, rows};
use dc_ui_kit::strings::inspector as names;
use dc_ui_kit::tokens::{ACCENT_HI, PAUSE, TEXT_3, WHITE};
use serde_json::{Map, Value, json};

use crate::{
    CLICK_SCALE_MAX, CLICK_SCALE_MIN, CLICK_TIME_MAX, CLICK_TIME_MIN, ClickKind, ClickStyle,
};

const KINDS: [(ClickKind, &str); 5] = [
    (ClickKind::Off, names::CLICK_OFF),
    (ClickKind::Ripple, names::CLICK_RIPPLE),
    (ClickKind::Spotlight, names::CLICK_SPOTLIGHT),
    (ClickKind::Echo, names::CLICK_ECHO),
    (ClickKind::Flash, names::CLICK_FLASH),
];

/// The v1 palette (docs/porting/clicks-callout.md §1) as far as the row holds it.
const COLORS: [(Rgba, &str); 5] = [
    (BLUE, names::BLUE),
    (Rgba([0xef, 0x44, 0x44, 0xff]), names::RED),
    (Rgba([0xf5, 0x9e, 0x0b, 0xff]), names::AMBER),
    (Rgba([0x22, 0xc5, 0x5e, 0xff]), names::GREEN),
    (Rgba([0xa8, 0x55, 0xf7, 0xff]), names::PURPLE),
];

/// Rays of the «Вспышка» picture.
const RAYS: usize = 8;

/// Tiles, colour, size of `style`; `edit` gets the field and the new value of a committed edit.
pub(crate) fn look(ui: &mut Ui, style: &ClickStyle, mut edit: impl FnMut(&str, Value)) {
    if let Some(kind) = rows::tiles(ui, style.kind, &KINDS, picture) {
        edit("style", json!(kind));
    }
    let own = names::OWN_COLOR;
    if let Some(color) = rows::palette(ui, names::CLICK_COLOR, style.color, &COLORS, own) {
        edit("color", json!(color));
    }
    let range = CLICK_SCALE_MIN..=CLICK_SCALE_MAX;
    let unit = (0.05, names::TIMES);
    if let Some(v) = rows::slider(ui, names::CLICK_SIZE, style.scale, range, unit) {
        edit("scale", json!(v));
    }
}

/// Opacity in percent and duration of `style`.
pub(crate) fn timing(ui: &mut Ui, style: &ClickStyle, mut edit: impl FnMut(&str, Value)) {
    let percent = (1.0, names::PERCENT);
    let opacity = style.opacity * 100.0;
    if let Some(v) = rows::slider(ui, names::CLICK_OPACITY, opacity, 0.0..=100.0, percent) {
        edit("opacity", json!(v / 100.0));
    }
    let range = CLICK_TIME_MIN.as_secs_f64()..=CLICK_TIME_MAX.as_secs_f64();
    let secs = (0.01, names::SECONDS);
    let duration = style.duration.as_secs_f64();
    if let Some(v) = rows::slider(ui, names::CLICK_DURATION, duration, range, secs) {
        edit("duration", json!(v));
    }
}

/// The fields of [`look`] as arguments of `clicks` and `click set`.
pub(crate) fn look_args(style: &ClickStyle) -> Map<String, Value> {
    args([
        ("style", json!(style.kind)),
        ("color", json!(style.color)),
        ("scale", json!(style.scale)),
    ])
}

/// The fields of [`timing`] as arguments of `clicks` and `click set`.
pub(crate) fn timing_args(style: &ClickStyle) -> Map<String, Value> {
    args([
        ("opacity", json!(style.opacity)),
        ("duration", json!(style.duration.as_secs_f64())),
    ])
}

/// The picture of a tile (mockup `.tile .pv`, 26 px): a cross, a blue ring, a white glow, two
/// rings, an amber flash with rays.
fn picture(painter: &Painter, rect: Rect, kind: ClickKind) {
    let centre = rect.center();
    let r = rect.width() / 2.0;
    match kind {
        ClickKind::Off => {
            let icon = Rect::from_center_size(centre, vec2(14.0, 14.0));
            Icon::Close.paint(painter, icon, TEXT_3);
        }
        ClickKind::Ripple => {
            painter.circle_stroke(centre, r - 2.0, Stroke::new(2.0, ACCENT_HI));
        }
        ClickKind::Spotlight => {
            for step in (1..=6).rev() {
                let share = step as f32 / 6.0;
                let alpha = (1.0 - share) * 0.5 + 0.08;
                painter.circle_filled(centre, r * share * 0.75, WHITE.gamma_multiply(alpha));
            }
        }
        ClickKind::Echo => {
            painter.circle_stroke(centre, r - 4.0, Stroke::new(2.0, WHITE));
            painter.circle_stroke(centre, r - 1.5, Stroke::new(2.5, WHITE.gamma_multiply(0.4)));
        }
        ClickKind::Flash => flash(painter, centre, r),
    }
}

fn flash(painter: &Painter, centre: Pos2, r: f32) {
    let amber: Color32 = PAUSE;
    painter.circle_filled(centre, r * 0.3, amber);
    for i in 0..RAYS {
        let angle = i as f32 * std::f32::consts::TAU / RAYS as f32;
        let along = vec2(angle.cos(), angle.sin());
        let line = [centre + along * r * 0.45, centre + along * r];
        painter.line_segment(line, Stroke::new(2.0, amber));
    }
}
