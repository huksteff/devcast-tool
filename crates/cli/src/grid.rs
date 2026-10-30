//! `devcast frame --grid`: lines every 0.1 of the frame with their values, over everything and not
//! moved by the zoom camera, so an agent reads the fractions `--rect`, `--x/--y` and drawing
//! points take (annotation boxes are fractions of the frame before the camera: they match the
//! grid where no zoom is on).

use dc_render::{
    Align, Band, Beside, Font, Geometry, Layer, Paint, Plate, PxRect, Rgba, Scene, Shape, Text,
    TextRun,
};

const STEPS: u16 = 10;
const SHADE: Rgba = Rgba([0x00, 0x00, 0x00, 0x80]);
const LINE: Rgba = Rgba([0xff, 0xff, 0xff, 0xc0]);
const LABEL: Rgba = Rgba([0xff, 0xff, 0xff, 0xff]);

/// Puts the grid on top of `scene`; sizes are px at 1080 and grow with the frame.
pub(crate) fn draw(scene: &mut Scene) {
    let (w, h) = (scene.size.width as f32, scene.size.height as f32);
    let k = w.min(h) / 1080.0;
    let mut shapes = Vec::new();
    let mut labels = Vec::new();
    for i in 1..STEPS {
        let f = f32::from(i) / f32::from(STEPS);
        let width = if i * 2 == STEPS { 2.0 } else { 1.0 };
        for (from, to) in [([f * w, 0.0], [f * w, h]), ([0.0, f * h], [w, f * h])] {
            shapes.push(line(from, to, (width + 2.0) * k, SHADE));
            shapes.push(line(from, to, width * k, LINE));
        }
        let value = format!("0.{i}");
        labels.push(label(&value, [f * w, 0.0], w, h, k));
        labels.push(label(&value, [0.0, f * h], w, h, k));
    }
    scene.push(Band::Overlay, Layer::Shapes(shapes));
    for text in labels {
        scene.push(Band::Overlay, Layer::Text(text));
    }
}

fn line(from: [f32; 2], to: [f32; 2], width: f32, color: Rgba) -> Shape {
    Shape {
        geometry: Geometry::Line { from, to, width },
        fill: Some(Paint::Solid(color)),
        stroke: None,
        opacity: 1.0,
        rotation: 0.0,
        glow: None,
        drawn: 1.0,
    }
}

/// The value of a line by its end at the top or the left edge.
fn label(value: &str, point: [f32; 2], w: f32, h: f32, k: f32) -> Text {
    Text {
        rect: PxRect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
        runs: vec![TextRun {
            text: value.to_owned(),
            color: LABEL,
        }],
        font: Font {
            family: "Inter".to_owned(),
            size: 20.0 * k,
            bold: true,
            italic: false,
        },
        underline: false,
        align: Align::Left,
        line_height: 1.4,
        padding: 0.0,
        plate: Some(Plate {
            paint: Paint::Solid(Rgba([0x00, 0x00, 0x00, 0xb0])),
            radius: 4.0 * k,
            pad: [5.0 * k, 2.0 * k],
            glow: None,
        }),
        opacity: 1.0,
        beside: Some(Beside {
            point,
            offset: [6.0 * k, 6.0 * k],
            margin: 0.0,
        }),
        shadow: None,
        rotation: 0.0,
    }
}
