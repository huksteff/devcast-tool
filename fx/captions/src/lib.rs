//! Captions: cues, style, animation.

mod edit;
mod generate;
mod model;
mod retime;
mod span;
mod text;

pub use edit::{CaptionsClear, CaptionsMerge, CaptionsRm, CaptionsSet, CaptionsSplit};
pub use generate::{CaptionsGen, NewCue, merge_tracks};
pub use model::{CaptionsEffect, CaptionsState, Cue, CueId, CueKind, Word};
pub use text::{CaptionsImport, export_text, parse_text};

use dc_core::{EffectInfo, IdKind, OpInfo};

/// All caption commands of the registry, in the order `devcast help` lists them. `captions gen`,
/// `export` and `import` need files and Whisper, so the CLI runs them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<CaptionsSet>(),
        OpInfo::of::<CaptionsSplit>(),
        OpInfo::of::<CaptionsMerge>(),
        OpInfo::of::<CaptionsRm>(),
        OpInfo::of::<CaptionsClear>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<CaptionsEffect>(vec![CueKind::PREFIX])
}

#[cfg(test)]
mod retime_tests;
#[cfg(test)]
mod tests;
