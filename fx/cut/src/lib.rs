//! Clips on the main track, cuts and speed ranges, and their picture in the scene. Every time
//! argument is output time; commands turn it into clip source time through the time map.

mod clip;
mod cut;
mod evaluate;
pub mod lane;
pub mod look;
mod speed;
mod split;
mod track;

#[cfg(test)]
mod test_util;

use dc_core::OpInfo;

pub use clip::{ClipAdd, ClipMove, ClipRm};
pub use cut::Cut;
pub use evaluate::evaluate;
pub use speed::Speed;
pub use split::ClipSplit;

pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<Cut>(),
        OpInfo::of::<Speed>(),
        OpInfo::of::<ClipRm>(),
        OpInfo::of::<ClipMove>(),
        OpInfo::of::<ClipSplit>(),
    ]
}
