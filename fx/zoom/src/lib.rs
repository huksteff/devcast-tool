//! Zoom regions, follow mode, motion, auto zooms; the inspector panels of the zoom are [`panel`].

mod aim;
mod classify;
mod ease;
mod follow;
mod frame;
pub mod lane;
pub mod look;
mod model;
mod motion;
pub mod panel;
mod panel_region;
mod pointer;
mod ramp;
mod region;
mod shot;
mod span;
mod spring;
pub mod stage;
mod suggest;
mod track;

pub use frame::{Corner, frame, reachable, resize};
pub use model::{
    DEFAULT_SCALE, MOTION_BLUR_MAX, Mode, Preset, SCALE_MAX, SCALE_MIN, TRANSITION_MAX,
    TRANSITION_MIN, Zoom, ZoomEffect, ZoomId, ZoomKind, ZoomSettings, ZoomState,
};
pub use motion::ZoomMotion;
pub use region::{ZoomAdd, ZoomRm, ZoomSet};
pub use suggest::{
    AT_CLICKS, Action, CLUSTER_GAP, CLUSTER_PAD, Pointer, Recording, Suggestion, Suggestions,
    by_clicks, suggest,
};
pub use track::{CameraTrack, STEP};

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
mod panel_tests;
#[cfg(test)]
mod suggest_tests;
#[cfg(test)]
mod tests;
