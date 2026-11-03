//! Captions: cues, style, animation; the inspector panels of the captions are [`panel`].

mod edit;
mod evaluate;
mod generate;
pub mod lane;
mod layout;
mod model;
pub mod panel;
mod panel_cue;
mod panel_recognition;
mod panel_style;
mod paste;
mod retime;
mod silence;
mod span;
mod style;
mod style_diff;
mod text;

pub use edit::{CaptionsClear, CaptionsMerge, CaptionsRm, CaptionsSet, CaptionsSplit};
pub use evaluate::evaluate;
pub use generate::{CaptionsGen, NewCue, merge_tracks};
pub use layout::Prepared;
pub use model::{CaptionsEffect, CaptionsState, Cue, CueId, CueKind, Word};
pub use paste::CaptionsPaste;
pub use silence::{Auto, SILENCE_MAX, SILENCE_MIN, Silence};
pub use style::{
    Animation, BOTTOM_MAX, CaptionsSettings, CaptionsStyle, RADIUS_MAX, ROWS_MAX, SIZE_MAX,
    SIZE_MIN, WIDTH_MAX, WIDTH_MIN,
};
pub use text::{CaptionsImport, export_text, parse_text};

use dc_core::{EffectInfo, IdKind, OpInfo};

/// All caption commands of the registry, in the order `devcast help` lists them. `captions gen`,
/// `export` and `import` need files and Whisper, so the CLI runs them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<CaptionsSet>(),
        OpInfo::of::<CaptionsSplit>(),
        OpInfo::of::<CaptionsMerge>(),
        OpInfo::of::<CaptionsPaste>(),
        OpInfo::of::<CaptionsRm>(),
        OpInfo::of::<CaptionsClear>(),
        OpInfo::of::<CaptionsStyle>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<CaptionsEffect>(vec![CueKind::PREFIX])
}

#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod retime_tests;
#[cfg(test)]
mod tests;
