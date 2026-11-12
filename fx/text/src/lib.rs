//! Text annotation: what it says and how (font, size, style, alignment, colours, plate), the
//! `text set` command and the text layer of the scene (docs/porting/annotations.md §5).

mod commands;
mod evaluate;
pub mod look;
mod model;
mod panel;

pub use commands::TextSet;
pub use evaluate::evaluate;
pub use model::{RADIUS_MAX, SIZE_MAX, SIZE_MIN, TEXT_MAX, TextAlign, TextBody};

use dc_core::OpInfo;

/// Commands of text annotations, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<TextSet>()]
}

#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
