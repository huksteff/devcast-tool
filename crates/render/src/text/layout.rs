//! Plates and underlines of a laid-out text, as figures of the shape pass.

use glyphon::{Buffer, Color, LayoutRun};

use crate::scene::{Geometry, Paint, PxRect, Rgba, Shape, Text};

use super::Decor;

/// Plates behind the ink of each line and underlines of each colour, for a buffer drawn at
/// `origin` and cut by `clip` (target px). `k` is target px per frame px of the text's own
/// lengths (camera included); the figures come out in frame px of the target, `scale` target px
/// per frame px.
pub(super) fn decor(
    buffer: &Buffer,
    text: &Text,
    origin: [f32; 2],
    clip: PxRect,
    k: f32,
    scale: f32,
) -> Decor {
    let mut decor = Decor {
        plates: Vec::new(),
        underlines: Vec::new(),
    };
    let to_frame = |r: PxRect| {
        cut(r, clip).map(|r| PxRect {
            x: r.x / scale,
            y: r.y / scale,
            w: r.w / scale,
            h: r.h / scale,
        })
    };
    for run in buffer.layout_runs() {
        if let (Some(plate), Some([x0, x1])) = (&text.plate, ink(&run)) {
            let [px, py] = plate.pad.map(|p| p * k);
            let rect = PxRect {
                x: origin[0] + x0 - px,
                y: origin[1] + run.line_top - py,
                w: x1 - x0 + 2.0 * px,
                h: run.line_height + 2.0 * py,
            };
            if let Some(rect) = to_frame(rect) {
                let color = faded(plate.color, text.opacity);
                let radius = plate.radius * k / scale;
                decor.plates.push(figure(rect, radius, color));
            }
        }
        for (x0, x1, y, thickness, color) in underlines(&run) {
            let rect = PxRect {
                x: origin[0] + x0,
                y: origin[1] + y - thickness / 2.0,
                w: x1 - x0,
                h: thickness,
            };
            if let Some(rect) = to_frame(rect) {
                decor.underlines.push(figure(rect, 0.0, color));
            }
        }
    }
    decor
}

/// Left and right of the line's glyphs without spaces at its ends.
fn ink(run: &LayoutRun<'_>) -> Option<[f32; 2]> {
    let visible =
        |g: &&glyphon::LayoutGlyph| !run.text[g.start..g.end].chars().all(char::is_whitespace);
    let left = run
        .glyphs
        .iter()
        .filter(visible)
        .map(|g| g.x)
        .reduce(f32::min)?;
    let right = run
        .glyphs
        .iter()
        .filter(visible)
        .map(|g| g.x + g.w)
        .reduce(f32::max)?;
    Some([left, right])
}

/// Underline pieces of a line, one per stretch of one colour: left, right, centre y (from the
/// top of the text), thickness, colour. Offset and thickness are the font's.
fn underlines(run: &LayoutRun<'_>) -> Vec<(f32, f32, f32, f32, Rgba)> {
    let mut pieces: Vec<(f32, f32, f32, f32, Rgba)> = Vec::new();
    for span in run.decorations {
        let metrics = span.data.underline_metrics;
        let y = run.line_y - metrics.offset * span.font_size;
        let thickness = (metrics.thickness * span.font_size).max(1.0);
        for glyph in &run.glyphs[span.glyph_range.clone()] {
            let color = rgba(glyph.color_opt.or(span.color_opt));
            match pieces.last_mut() {
                Some(last) if last.4 == color && (last.1 - glyph.x).abs() < 0.5 => {
                    last.1 = glyph.x + glyph.w;
                }
                _ => pieces.push((glyph.x, glyph.x + glyph.w, y, thickness, color)),
            }
        }
    }
    pieces
}

fn rgba(color: Option<Color>) -> Rgba {
    Rgba(color.map_or([255; 4], |c| c.as_rgba()))
}

fn faded(color: Rgba, opacity: f32) -> Rgba {
    let [r, g, b, a] = color.0;
    Rgba([r, g, b, (f32::from(a) * opacity).round() as u8])
}

/// The part of `rect` inside `clip`; `None` when nothing is.
fn cut(rect: PxRect, clip: PxRect) -> Option<PxRect> {
    let x0 = rect.x.max(clip.x);
    let y0 = rect.y.max(clip.y);
    let x1 = (rect.x + rect.w).min(clip.x + clip.w);
    let y1 = (rect.y + rect.h).min(clip.y + clip.h);
    (x0 < x1 && y0 < y1).then_some(PxRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

fn figure(rect: PxRect, radius: f32, color: Rgba) -> Shape {
    Shape {
        geometry: Geometry::Rect { rect, radius },
        fill: Some(Paint::Solid(color)),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
    }
}
