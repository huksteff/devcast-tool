//! The pressed-keys plate in the scene: the shortcut on screen at an output moment, with its
//! count, over everything and outside the zoom camera.

use dc_core::{OutputTime, TimeDelta};
use dc_render::{Align, Band, Font, Layer, Paint, Plate, PxRect, Scene, Text, TextRun};

use crate::prepared::{Prepared, Shown};
use crate::state::Position;

/// Font size in px at size 1 when the short side of the frame is `REFERENCE` px.
const FONT_PX: f64 = 36.0;
const REFERENCE: f64 = 1080.0;
const FAMILY: &str = "Inter";
/// Distance between the plate and the frame edges, fractions of the short side.
const MARGIN: f64 = 0.04;
/// The plate around the letters and its corners, font sizes.
const PAD: [f64; 2] = [0.5, 0.25];
const RADIUS: f64 = 0.3;
const LINE_HEIGHT: f32 = 1.4;
const FADE_IN: TimeDelta = TimeDelta::from_millis(100);
const FADE_OUT: TimeDelta = TimeDelta::from_millis(250);

/// Puts the shortcut shown at output moment `t` into `scene`, in `Band::Overlay`.
pub fn evaluate(prepared: &Prepared, t: OutputTime, scene: &mut Scene) {
    let settings = &prepared.settings;
    if !settings.show {
        return;
    }
    let after = prepared.shown.partition_point(|s| s.presses[0] <= t);
    let Some(shown) = after.checked_sub(1).map(|i| &prepared.shown[i]) else {
        return;
    };
    if t >= shown.end {
        return;
    }
    let count = shown.presses.partition_point(|p| *p <= t);
    let text = match count {
        1 => shown.name.clone(),
        n => format!("{} ×{n}", shown.name),
    };
    let (w, h) = (f64::from(scene.size.width), f64::from(scene.size.height));
    let short = w.min(h);
    let font = FONT_PX * settings.size * short / REFERENCE;
    let margin = MARGIN * short;
    let [pad_x, pad_y] = PAD.map(|p| p * font);
    let height = font * f64::from(LINE_HEIGHT) + 2.0 * pad_y;
    let position = settings.position;
    let y = match position.top() {
        true => margin,
        false => h - margin - height,
    };
    let layer = Text {
        rect: PxRect {
            x: margin as f32,
            y: y as f32,
            w: (w - 2.0 * margin) as f32,
            h: height as f32,
        },
        runs: vec![TextRun {
            text,
            color: settings.color,
        }],
        font: Font {
            family: FAMILY.to_owned(),
            size: font as f32,
            bold: true,
            italic: false,
        },
        underline: false,
        align: align(position),
        line_height: LINE_HEIGHT,
        padding: pad_x as f32,
        plate: Some(Plate {
            paint: Paint::Solid(settings.background),
            radius: (RADIUS * font) as f32,
            pad: [pad_x as f32, pad_y as f32],
            glow: None,
        }),
        opacity: opacity(shown, t),
        beside: None,
    };
    scene.push(Band::Overlay, Layer::Text(layer));
}

fn align(position: Position) -> Align {
    match position {
        Position::TopLeft | Position::BottomLeft => Align::Left,
        Position::Top | Position::Bottom => Align::Center,
        Position::TopRight | Position::BottomRight => Align::Right,
    }
}

/// Linear fade in after the first press and out before the end, unless another shortcut is
/// right next to it.
fn opacity(shown: &Shown, t: OutputTime) -> f32 {
    let ramp = |left: TimeDelta, fade: TimeDelta| left.as_secs_f64() / fade.as_secs_f64();
    let fade_in = match shown.fade_in {
        true => ramp(t.since(shown.presses[0]), FADE_IN),
        false => 1.0,
    };
    let fade_out = match shown.fade_out {
        true => ramp(shown.end.since(t), FADE_OUT),
        false => 1.0,
    };
    fade_in.min(fade_out).min(1.0) as f32
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;
