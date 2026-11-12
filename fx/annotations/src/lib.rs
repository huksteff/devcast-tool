//! Shared annotation model: tracks, layers, box, rotation, time, entrance and exit animations,
//! keyframes; the kinds (text, shape, image, drawing, blur) keep their own fields in
//! `Annotation::body` and live in their own crates.

mod animate;
mod arrange;
mod boxes;
mod commands;
mod keyed;
mod keys;
mod kinds;
pub mod lane;
mod lane_tip;
pub mod look;
mod model;
mod motion;
pub mod panel;
mod panel_rows;
mod place;
mod span;
pub mod stage;

pub use animate::AnnotAnim;
pub use arrange::{AnnotLayer, AnnotMove, AnnotPaste, Step};
pub use commands::{AnnotAdd, AnnotRm, AnnotSet};
pub use keyed::{
    AnnotKeyAdd, AnnotKeyCopy, AnnotKeyMove, AnnotKeyRm, edit_body, edit_kind, key_offset,
};
pub use keys::{Blend, KEY_TOLERANCE, Key, keyed, same_moment, step};
pub use model::{
    AnnotId, AnnotKind, Annotation, Annotations, AnnotationsState, Kind, MIN_DURATION,
    POSITION_LIMIT, ROTATION_LIMIT, SIZE_MAX, SIZE_MIN, TRACKS,
};
pub use motion::{Animation, Pose, STAGE_DEFAULT, STAGE_MAX, Stage, shown_for};
pub use place::Placement;

use dc_core::{EffectInfo, IdKind, OpInfo};

/// All shared annotation commands, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<AnnotAdd>(),
        OpInfo::of::<AnnotSet>(),
        OpInfo::of::<AnnotAnim>(),
        OpInfo::of::<AnnotKeyAdd>(),
        OpInfo::of::<AnnotKeyRm>(),
        OpInfo::of::<AnnotKeyCopy>(),
        OpInfo::of::<AnnotKeyMove>(),
        OpInfo::of::<AnnotMove>(),
        OpInfo::of::<AnnotPaste>(),
        OpInfo::of::<AnnotLayer>(),
        OpInfo::of::<AnnotRm>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Annotations>(vec![AnnotKind::PREFIX])
}

#[cfg(test)]
mod arrange_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod motion_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
