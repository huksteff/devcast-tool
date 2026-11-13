//! Shape annotation: arrow, rectangle, ellipse, circle, line, highlight; the `shape set` command
//! and the figure in the scene (docs/porting/annotations.md §6).

mod commands;
mod evaluate;
pub mod look;
mod model;
pub mod panel;

pub use commands::ShapeSet;
pub use evaluate::evaluate;
pub use model::{
    BoxPoint, CYCLE_MAX, CYCLE_MIN, Figure, GLOW_MAX, HIGHLIGHT_OPACITY, RADIUS_MAX, ShapeBody,
    WIDTH_MAX, WIDTH_MIN,
};

use dc_core::OpInfo;

/// Commands of shape annotations, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<ShapeSet>()]
}

#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
