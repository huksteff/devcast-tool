//! Blur annotation: a region that blurs what lies below it, with a tint from none to a solid
//! «censor» fill; the `blur set` command and the region in the scene
//! (docs/porting/annotations.md §9, decision 000).

mod commands;
mod evaluate;
mod model;

pub use commands::BlurSet;
pub use evaluate::evaluate;
pub use model::{BlurBody, RADIUS_MAX, STRENGTH_MAX, STRENGTH_MIN};

use dc_core::OpInfo;

/// Commands of blur annotations, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<BlurSet>()]
}

#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod tests;
