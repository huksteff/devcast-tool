//! Drawing annotation: brush strokes of a pencil and an eraser in the annotation box, a straight
//! line being a stroke of two points; the `drawing …` commands and the layer in the scene
//! (docs/porting/annotations.md §8).

mod commands;
mod evaluate;
pub mod look;
mod model;
mod panel;
pub mod stage;

pub use commands::{DrawingAdd, DrawingClear, DrawingRm, DrawingSet};
pub use evaluate::evaluate;
pub use model::{
    DrawingBody, ERASER_WIDTH, PENCIL_WIDTH, Stroke, StrokeTool, WIDTH_MAX, WIDTH_MIN,
};

use dc_core::OpInfo;

/// Commands of drawing annotations, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<DrawingAdd>(),
        OpInfo::of::<DrawingSet>(),
        OpInfo::of::<DrawingRm>(),
        OpInfo::of::<DrawingClear>(),
    ]
}

#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
