//! Zoom regions, follow mode, motion, auto zooms.

mod model;
mod motion;
mod region;
mod span;

pub use model::{
    DEFAULT_SCALE, MOTION_BLUR_MAX, Mode, Preset, SCALE_MAX, SCALE_MIN, TRANSITION_MAX,
    TRANSITION_MIN, Zoom, ZoomEffect, ZoomId, ZoomKind, ZoomSettings, ZoomState,
};
pub use motion::ZoomMotion;
pub use region::{ZoomAdd, ZoomRm, ZoomSet};

use dc_core::{EffectInfo, IdKind, OpInfo};

/// All zoom commands, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<ZoomAdd>(),
        OpInfo::of::<ZoomSet>(),
        OpInfo::of::<ZoomRm>(),
        OpInfo::of::<ZoomMotion>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<ZoomEffect>(vec![ZoomKind::PREFIX])
}

#[cfg(test)]
mod tests;
