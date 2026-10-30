//! Captions in the scene (docs/porting/captions.md §6.4–§8, decision 000): the page of the cue
//! being said, its words coloured by what was said, on a rounded black plate over the bottom of
//! the frame, coming and going by output time.

use dc_core::time::Source;
use dc_core::{OutputTime, Range, SourceTime, TimeDelta, TimeMap};
use dc_render::{
    Align, Band, Font, Geometry, Layer, Paint, PxRect, Rgba, Scene, Shadow, Shape, Text, TextRun,
};

use crate::layout::{LINE_HEIGHT, Laid, PAD, Page, Prepared};
use crate::style::{Animation, CaptionsSettings};

/// A gap shorter than this keeps the caption on screen until the next cue (v1).
const GAP: TimeDelta = TimeDelta::from_millis(500);
/// Coming and going, output time (v1, linear).
const FADE: f64 = 0.18;
/// Corner radius and animation travel are given at this font size.
const BASE_SIZE: f64 = 30.0;
/// The line box is this much wider than the widest line, so the text pass never wraps it again.
const SLACK: f64 = 2.0;

/// Puts the caption shown at output moment `t` into `scene`, in `Band::Overlay`.
pub fn evaluate(prepared: &Prepared, map: &TimeMap, t: OutputTime, scene: &mut Scene) {
    let Some(segment) = map.segment_at(t) else {
        return;
    };
    let now = segment.source_at(t);
    let Some((index, cue)) = shown(prepared, &segment.media, now) else {
        return;
    };
    let page_at = cue.pages.partition_point(|p| p.start <= now).max(1) - 1;
    let page = &cue.pages[page_at];
    let leaves = page_at + 1 == cue.pages.len() && !followed(prepared, index);
    let [v, exit] = visibility(map, cue, page, now, t, leaves);
    let look = look(prepared.settings.animation, v, leaves.then_some(exit));
    push(prepared, cue, page, now, look, scene);
}

/// The cue on screen at source `now` of `media`: the latest one started, kept until the next
/// one when the gap to it is short.
fn shown<'a>(prepared: &'a Prepared, media: &str, now: SourceTime) -> Option<(usize, &'a Laid)> {
    let cues = &prepared.cues;
    let mut found = None;
    for (i, cue) in cues.iter().enumerate() {
        if cue.media != media || cue.range.start() > now {
            continue;
        }
        let end = match cues.get(i + 1).filter(|n| n.media == cue.media) {
            Some(next) if next.range.start().since(cue.range.end()) < GAP => {
                next.range.start().max(cue.range.end())
            }
            _ => cue.range.end(),
        };
        if now < end {
            found = Some((i, cue));
        }
    }
    found
}

/// The cue is not followed closely by another: its last page fades out after its words.
fn followed(prepared: &Prepared, index: usize) -> bool {
    let cue = &prepared.cues[index];
    prepared
        .cues
        .get(index + 1)
        .is_some_and(|n| n.media == cue.media && n.range.start().since(cue.range.end()) < GAP)
}

/// `[v, exit]`, 0..1 by output time (sped-up or cut parts do not hurry it, decision 000): how far
/// the page has come in and not yet gone, and how far it has not yet gone.
fn visibility(
    map: &TimeMap,
    cue: &Laid,
    page: &Page,
    now: SourceTime,
    t: OutputTime,
    leaves: bool,
) -> [f64; 2] {
    let output = |from: SourceTime, to: SourceTime| -> f64 {
        Range::<Source>::new(from, to)
            .ok()
            .and_then(|r| map.output_span(&cue.media, r))
            .map_or(0.0, |span| span.end().since(span.start()).as_secs_f64())
    };
    let shown = Range::<Source>::new(page.start, now)
        .ok()
        .and_then(|r| map.output_span(&cue.media, r))
        .map_or(t, |span| span.start());
    let enter = t.since(shown).as_secs_f64() / FADE;
    let words = &cue.words[cue.lines[page.lines.start].start..cue.lines[page.lines.end - 1].end];
    let said = words
        .iter()
        .map(|w| w.range.end())
        .max()
        .unwrap_or(page.end)
        .max(page.start);
    let anchor = if leaves { said } else { page.end };
    let exit = if now < anchor {
        output(now, anchor) / FADE
    } else {
        0.0
    };
    let exit = exit.clamp(0.0, 1.0);
    [enter.clamp(0.0, 1.0).min(exit), exit]
}

/// What the animation does at visibility `v`: opacity, downward shift and scale (shift in font
/// sizes / 30, so it grows with the text).
struct Look {
    opacity: f64,
    shift: f64,
    scale: f64,
}

/// v1 `captionLayout.ts:515-552`; a page that leaves fades fully out.
fn look(animation: Animation, v: f64, leaving: Option<f64>) -> Look {
    let (floor, shift, scale) = match animation {
        Animation::None => (1.0, 0.0, 1.0),
        Animation::Fade => (0.3 + 0.7 * v, 0.0, 1.0),
        Animation::Rise => (0.25 + 0.75 * v, (1.0 - v) * 18.0, 0.985 + 0.015 * v),
        Animation::Pop => (0.35 + 0.65 * v, (1.0 - v) * 8.0, 0.94 + 0.06 * v),
    };
    let fade_out = leaving.filter(|_| animation != Animation::None);
    Look {
        opacity: fade_out.map_or(floor, |exit| floor * exit),
        shift,
        scale,
    }
}

/// The plate and the text of `page`, centred over the bottom of the frame.
fn push(
    prepared: &Prepared,
    cue: &Laid,
    page: &Page,
    now: SourceTime,
    look: Look,
    scene: &mut Scene,
) {
    let settings = &prepared.settings;
    let size = f64::from(prepared.font.size);
    let (w, h) = (f64::from(scene.size.width), f64::from(scene.size.height));
    let lines = page.lines.end - page.lines.start;
    let widest = cue.widths[page.lines.clone()]
        .iter()
        .copied()
        .fold(0.0, f64::max);
    let s = look.scale;
    let (pad_x, pad_y) = (PAD[0] * size, PAD[1] * size);
    let box_w = (widest + SLACK + 2.0 * pad_x) * s;
    let box_h = (lines as f64 * LINE_HEIGHT * size + 2.0 * pad_y) * s;
    let centre_y =
        h - h * settings.bottom / 100.0 - (lines as f64 * LINE_HEIGHT * size + 2.0 * pad_y) / 2.0
            + look.shift * size / BASE_SIZE;
    let rect = PxRect {
        x: ((w - box_w) / 2.0) as f32,
        y: (centre_y - box_h / 2.0) as f32,
        w: box_w as f32,
        h: box_h as f32,
    };
    if settings.background > 0.0 {
        let alpha = (settings.background * 255.0).round() as u8;
        scene.push(
            Band::Overlay,
            Layer::Shapes(vec![Shape {
                geometry: Geometry::Rect {
                    rect,
                    radius: (settings.radius * size / BASE_SIZE * s) as f32,
                },
                fill: Some(Paint::Solid(Rgba([0, 0, 0, alpha]))),
                stroke: None,
                opacity: look.opacity as f32,
                rotation: 0.0,
                glow: None,
                drawn: 1.0,
            }]),
        );
    }
    scene.push(
        Band::Overlay,
        Layer::Text(Text {
            rect,
            runs: runs(settings, cue, page, now),
            font: Font {
                size: (size * s) as f32,
                ..prepared.font.clone()
            },
            underline: false,
            align: Align::Center,
            line_height: LINE_HEIGHT as f32,
            padding: (pad_x * s) as f32,
            plate: None,
            opacity: look.opacity as f32,
            beside: None,
            shadow: settings.shadow.then(|| shadow(size * s)),
            rotation: 0.0,
        }),
    );
}

/// v1 text shadow at font size `size`: `rgba(0,0,0,0.72)`, blur `max(2, 0.16·fs)` (= 2σ), down
/// `max(1, 0.06·fs)`.
fn shadow(size: f64) -> Shadow {
    Shadow {
        color: Rgba([0, 0, 0, 184]),
        sigma: ((0.16 * size).max(2.0) / 2.0) as f32,
        offset: [0.0, (0.06 * size).max(1.0) as f32],
    }
}

/// The words of `page` line by line, coloured: said, being said, not yet (decision 000). Between
/// words the last one started stays highlighted; before the first, the first is (v1).
fn runs(settings: &CaptionsSettings, cue: &Laid, page: &Page, now: SourceTime) -> Vec<TextRun> {
    let active = cue.words.partition_point(|w| w.range.start() <= now).max(1) - 1;
    let mut runs = Vec::new();
    for (n, line) in cue.lines[page.lines.clone()].iter().enumerate() {
        for i in line.clone() {
            let lead = match (n, i == line.start) {
                (0, true) => "",
                (_, true) => "\n",
                _ => " ",
            };
            let color = match i.cmp(&active) {
                std::cmp::Ordering::Less => settings.color,
                std::cmp::Ordering::Equal => settings.highlight,
                std::cmp::Ordering::Greater => settings.inactive,
            };
            runs.push(TextRun {
                text: format!("{lead}{}", cue.words[i].text),
                color,
            });
        }
    }
    runs
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;
