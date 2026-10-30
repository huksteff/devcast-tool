//! The text layer of a text annotation (docs/porting/annotations.md §5, export layout of v1):
//! lines wrapped at the box width minus the padding, the block centred vertically, the box cuts
//! the text; a plate behind each line; the whole turned with the box.

use dc_fx_annotations::Placement;
use dc_render::{Font, Paint, Plate, Text, TextRun};

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
        runs: vec![TextRun {
            text: body.text.clone(),
            color: body.color,
        }],
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
        opacity: 1.0,
        beside: None,
        shadow: None,
        rotation: placement.rotation,
    }
}
