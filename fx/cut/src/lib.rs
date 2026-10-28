//! Clips on the main track, cuts and speed ranges. Every time argument is output time; commands
//! turn it into clip source time through the time map.

mod clip;
mod cut;
mod speed;
mod track;

#[cfg(test)]
mod test_util;

use dc_core::OpInfo;

pub use clip::{ClipMove, ClipRm};
pub use cut::Cut;
pub use speed::Speed;

/// Every command of the effect, for the ops registry.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<Cut>(),
        OpInfo::of::<Speed>(),
        OpInfo::of::<ClipRm>(),
        OpInfo::of::<ClipMove>(),
    ]
}
