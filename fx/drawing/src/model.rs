//! The fields of a drawing annotation, kept in `Annotation::body` (fx/annotations).

use dc_fx_annotations::{Blend, step};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Stroke thickness, px when the short side of the frame is 1080 px.
pub const WIDTH_MIN: f64 = 1.0;
pub const WIDTH_MAX: f64 = 64.0;
/// Thickness of a new pencil stroke and of a new eraser stroke (v1 `strokeWidth`, `eraserSize`).
pub const PENCIL_WIDTH: f64 = 4.0;
pub const ERASER_WIDTH: f64 = 12.0;

/// What a drawing annotation draws; a missing field takes its default (a new drawing is empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DrawingBody {
    /// Strokes in the order they were drawn: an eraser stroke clears what the earlier strokes of
    /// this drawing painted under it, never the video or other annotations.
    pub strokes: Vec<Stroke>,
    /// Opacity of the whole drawing, 0..1.
    pub opacity: f64,
}

impl Default for DrawingBody {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
            opacity: 1.0,
        }
    }
}

/// One stroke: a polyline with round joins and caps through its points (a single point is a
/// dot, two points a straight line). The box moves, stretches and turns it; its thickness does
/// not follow the box (decision 000).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Stroke {
    /// pencil paints, eraser clears.
    pub tool: StrokeTool,
    /// Colour of a pencil stroke, #rrggbbaa; an eraser stroke paints nothing with it.
    pub color: Rgba,
    /// Thickness, px when the short side of the frame is 1080 px, 1..64.
    pub width: f64,
    /// Points `[x, y]` in fractions of the box, each 0..1, in drawing order (draw-in follows
    /// it).
    pub points: Vec<[f64; 2]>,
}

/// What a stroke does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StrokeTool {
    /// Paints in its colour.
    Pencil,
    /// Clears what earlier strokes of the drawing painted under it.
    Eraser,
}

impl StrokeTool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pencil => "pencil",
            Self::Eraser => "eraser",
        }
    }

    /// Thickness of a new stroke of this tool.
    pub fn width(self) -> f64 {
        match self {
            Self::Pencil => PENCIL_WIDTH,
            Self::Eraser => ERASER_WIDTH,
        }
    }
}

/// Between two keys: the opacity blends; the strokes are a list and step at the right key whole,
/// as the key snapshot (`Blend for Value`) takes lists.
impl Blend for DrawingBody {
    fn blend(&self, to: &Self, e: f64) -> Self {
        Self {
            strokes: step(&self.strokes, &to.strokes, e),
            opacity: self.opacity.blend(&to.opacity, e),
        }
    }
}
