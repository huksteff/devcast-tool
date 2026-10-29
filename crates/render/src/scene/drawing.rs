//! Drawing layer: freehand brush strokes (drawing annotation, v1 `drawingData`).

use serde::{Deserialize, Serialize};

use super::Rgba;

/// Strokes painted in order into a layer of their own, so the eraser removes only earlier strokes
/// of this drawing, never what lies below; the layer is then laid over the frame with `opacity`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub strokes: Vec<BrushStroke>,
    /// 0..1: the part of the drawing's path shown, stroke after stroke at a constant speed in px
    /// (draw-in); 1 shows everything.
    pub drawn: f32,
    /// 0..1.
    pub opacity: f32,
}

/// A polyline with round joins and caps; one point is a dot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrushStroke {
    /// px of the frame.
    pub points: Vec<[f32; 2]>,
    /// Diameter, px of the frame.
    pub width: f32,
    pub tool: Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Pen(Rgba),
    /// Clears what earlier strokes painted under it.
    Eraser,
}
