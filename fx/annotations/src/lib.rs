//! Shared annotation model: tracks, layers, box, rotation, time, entrance and exit animations,
//! keyframes; the kinds (text, shape, image, drawing, blur) keep their own fields in
//! `Annotation::body` and live in their own crates.

mod animate;
mod arrange;
mod commands;
mod keyed;
mod keys;
mod model;
mod motion;
mod place;
mod span;

pub use animate::AnnotAnim;
pub use arrange::{AnnotLayer, AnnotMove, Step};
pub use commands::{AnnotAdd, AnnotRm, AnnotSet};
pub use keyed::{AnnotKeyAdd, AnnotKeyCopy, AnnotKeyRm, edit_body, key_offset};
pub use keys::{Blend, KEY_TOLERANCE, Key, keyed, step};
pub use model::{
    AnnotId, AnnotKind, Annotation, Annotations, AnnotationsState, Kind, MIN_DURATION,
    POSITION_LIMIT, ROTATION_LIMIT, SIZE_MAX, SIZE_MIN, TRACKS,
};
pub use motion::{Animation, Pose, STAGE_DEFAULT, STAGE_MAX, Stage, shown_for};
pub use place::{Placement, REFERENCE};

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
        OpInfo::of::<AnnotMove>(),
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
mod tests;
