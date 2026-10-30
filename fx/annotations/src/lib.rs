//! Shared annotation model: tracks, layers, box, rotation, time, entrance and exit animations;
//! the kinds (text, shape, image, drawing, blur) keep their own fields in `Annotation::body` and
//! live in their own crates.

mod animate;
mod arrange;
mod commands;
mod model;
mod motion;
mod place;
mod span;

pub use animate::AnnotAnim;
pub use arrange::{AnnotLayer, AnnotMove, Step};
pub use commands::{AnnotAdd, AnnotRm, AnnotSet};
pub use model::{
    AnnotId, AnnotKind, Annotation, Annotations, AnnotationsState, Kind, MIN_DURATION,
    POSITION_LIMIT, ROTATION_LIMIT, SIZE_MAX, SIZE_MIN, TRACKS,
};
pub use motion::{Animation, Pose, STAGE_DEFAULT, STAGE_MAX, Stage};
pub use place::{Placement, REFERENCE};

use dc_core::{EffectInfo, IdKind, OpInfo};

/// All shared annotation commands, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<AnnotAdd>(),
        OpInfo::of::<AnnotSet>(),
        OpInfo::of::<AnnotAnim>(),
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
mod motion_tests;
#[cfg(test)]
mod tests;
