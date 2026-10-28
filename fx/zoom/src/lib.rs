//! Zoom regions, follow mode, motion, auto zooms.

mod classify;
mod model;
mod motion;
mod region;
mod span;
mod suggest;

pub use model::{
    DEFAULT_SCALE, MOTION_BLUR_MAX, Mode, Preset, SCALE_MAX, SCALE_MIN, TRANSITION_MAX,
    TRANSITION_MIN, Zoom, ZoomEffect, ZoomId, ZoomKind, ZoomSettings, ZoomState,
};
pub use motion::ZoomMotion;
pub use region::{ZoomAdd, ZoomRm, ZoomSet};
pub use suggest::{
    Action, CLUSTER_GAP, CLUSTER_PAD, Pointer, Recording, Suggestion, Suggestions, suggest,
};

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
mod suggest_tests;
#[cfg(test)]
mod tests;
