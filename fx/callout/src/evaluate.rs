//! The callout bubble in the scene (docs/porting/clicks-callout.md §11, decision 000): a pill with
//! the typed text right and below the cursor tip, on the other side of it at a frame edge, under
//! the camera like the cursor.

use dc_core::{OutputTime, TimeMap};
use dc_render::{
    Align, Band, Beside, DEFAULT_FAMILY, Font, Glow, Gradient, Layer, Paint, Plate, PxRect, Rgba,
    Scene, Text, TextRun,
};

use crate::model::{CalloutSettings, CalloutState};
use crate::resolve::text_at;

/// Text height over the visible arrow height at size 1, and its floor in ref-px (v1 `fs`).
const FONT_PER_ARROW: f64 = 0.72;
const FONT_MIN: f64 = 16.0;
/// Padding around the text, font sizes, and its floors in ref-px (v1 `padX`, `padY`).
const PAD: [f64; 2] = [0.55, 0.32];
const PAD_MIN: [f64; 2] = [10.0, 7.0];
/// Corner of the bubble from the tip, arrow heights (v1).
const OFFSET: [f64; 2] = [0.60, 0.54];
/// The bubble keeps this far from the frame edges, ref-px.
const MARGIN: f64 = 16.0;
const LINE_HEIGHT: f32 = 1.2;
/// One loop of the gradient colours (v1: a full cycle in 2.4 s), output time.
const GRADIENT_LOOP_SECS: f64 = 2.4;
/// Glow σ per glow unit and ref-px (v1 figures: `shadowBlur = glow · 1.5` = 2σ).
const GLOW_SIGMA: f64 = 0.75;

/// Where the cursor is, from `dc_fx_cursor::pointer` (this crate does not depend on the cursor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    /// Px of the canvas, before the camera.
    pub tip: [f64; 2],
    /// Visible arrow height without the click bounce, px of the canvas.
    pub arrow: f64,
    /// Canvas px per ref-px (clip width on the canvas / 1920).
    pub k: f64,
}

/// Puts the callout of output moment `t` into `scene`, in `Band::Cursor` after the cursor. The
/// camera of `scene` must already be set: the frame edges are checked where the camera shows.
pub fn evaluate(
    state: &CalloutState,
    map: &TimeMap,
    t: OutputTime,
    anchor: Option<Anchor>,
    scene: &mut Scene,
) {
    let (Some(text), Some(anchor)) = (text_at(state, map, t), anchor) else {
        return;
    };
    let settings = &state.settings;
    let k = anchor.k;
    let font = (FONT_PER_ARROW * anchor.arrow * settings.size).max(FONT_MIN * k);
    let pad = [0, 1].map(|i| (PAD[i] * font).max(PAD_MIN[i] * k));
    let height = font * f64::from(LINE_HEIGHT) + 2.0 * pad[1];
    let layer = Text {
        rect: visible(scene),
        runs: vec![TextRun {
            text,
            color: settings.text_color,
        }],
        font: Font {
            family: DEFAULT_FAMILY.to_owned(),
            size: font as f32,
            bold: true,
            italic: false,
        },
        underline: false,
        align: Align::Left,
        line_height: LINE_HEIGHT,
        padding: 0.0,
        plate: Some(Plate {
            paint: paint(settings, t),
            radius: (height / 2.0) as f32,
            pad: pad.map(|p| p as f32),
            glow: glow(settings, k),
        }),
        opacity: 1.0,
        beside: Some(Beside {
            point: anchor.tip.map(|v| v as f32),
            offset: OFFSET.map(|o| (o * anchor.arrow) as f32),
            margin: (MARGIN * k) as f32,
        }),
        shadow: None,
        rotation: 0.0,
    };
    scene.push(Band::Cursor, Layer::Text(layer));
}

/// The part of the canvas the camera shows, px of the canvas.
fn visible(scene: &Scene) -> PxRect {
    let camera = scene.camera;
    let (w, h) = (scene.size.width as f32, scene.size.height as f32);
    PxRect {
        x: -camera.x / camera.scale,
        y: -camera.y / camera.scale,
        w: w / camera.scale,
        h: h / camera.scale,
    }
}

/// The gradient flows left to right in a seamless loop by output time, so a sped-up part does
/// not hurry it (decision 000).
fn paint(settings: &CalloutSettings, t: OutputTime) -> Paint {
    if !settings.gradient {
        return Paint::Solid(settings.background);
    }
    let secs = t.since(OutputTime::ZERO).as_secs_f64();
    Paint::Gradient(Gradient {
        colors: settings.colors.clone(),
        angle: 0.0,
        cycle: Some((secs / GRADIENT_LOOP_SECS).fract() as f32),
    })
}

/// Real glow (decision 000), with or without a gradient, in its first colour.
fn glow(settings: &CalloutSettings, k: f64) -> Option<Glow> {
    (settings.glow > 0.0).then(|| Glow {
        color: first(settings),
        sigma: (GLOW_SIGMA * settings.glow * k) as f32,
    })
}

fn first(settings: &CalloutSettings) -> Rgba {
    match settings.gradient {
        true => settings.colors.colors()[0],
        false => settings.background,
    }
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;
