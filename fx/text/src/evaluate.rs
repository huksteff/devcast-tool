//! The text layer of a text annotation (docs/porting/annotations.md §5, export layout of v1):
//! lines wrapped at the box width minus the padding, the block centred vertically, the box cuts
//! the text; a plate behind each line; the whole turned with the box.

use dc_fx_annotations::Placement;
use dc_render::{Font, Paint, Plate, Rgba, Text, TextRun};
use unicode_segmentation::UnicodeSegmentation;

use crate::model::TextBody;

/// Distance between baselines, font sizes (v1 `lh = 1.4·fs`).
const LINE_HEIGHT: f32 = 1.4;
/// Padding inside the box, reference px (v1 `pad = 8·k`).
const PADDING: f64 = 8.0;
/// Plate around the ink of a line, font sizes: v1 grows the line by 0.2·fs at the sides and
/// makes it 1.6·fs high, 0.1·fs over the 1.4·fs line.
const PLATE_PAD: [f64; 2] = [0.2, 0.1];

/// The text layer of `body` placed at `placement`, for `Band::Annotations` under the camera.
pub fn evaluate(body: &TextBody, placement: &Placement) -> Text {
    let k = placement.k;
    let size = body.size * k;
    let plate = (body.background.0[3] > 0).then(|| Plate {
        paint: Paint::Solid(body.background),
        radius: (body.radius * k) as f32,
        pad: PLATE_PAD.map(|p| (p * size) as f32),
        glow: None,
    });
    Text {
        rect: placement.rect,
        runs: runs(body, placement.reveal),
        font: Font {
            family: body.font.clone(),
            size: size as f32,
            bold: body.bold,
            italic: body.italic,
        },
        underline: body.underline,
        align: body.align.into(),
        line_height: LINE_HEIGHT,
        padding: (PADDING * k) as f32,
        plate,
        opacity: placement.opacity,
        beside: None,
        shadow: None,
        rotation: placement.rotation,
    }
}

/// The letters a typewriter has shown (`reveal` 0..1 of the grapheme clusters, rounded up as
/// v1) and the rest, laid out too but transparent: the whole text is wrapped once and words do
/// not jump to the next line while they are typed. A transparent letter is not ink, so the
/// plates grow with the typed part.
fn runs(body: &TextBody, reveal: f64) -> Vec<TextRun> {
    let count = body.text.graphemes(true).count();
    let shown = (count as f64 * reveal.clamp(0.0, 1.0)).ceil() as usize;
    let split = body
        .text
        .grapheme_indices(true)
        .nth(shown)
        .map_or(body.text.len(), |(at, _)| at);
    let (head, tail) = body.text.split_at(split);
    let hidden = Rgba([body.color.0[0], body.color.0[1], body.color.0[2], 0]);
    let mut runs = vec![TextRun {
        text: head.to_owned(),
        color: body.color,
    }];
    if !tail.is_empty() {
        runs.push(TextRun {
            text: tail.to_owned(),
            color: hidden,
        });
    }
    runs
}
