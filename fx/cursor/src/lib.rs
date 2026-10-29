//! The cursor of the recordings: the captured system pictures on a smoothed or exact path,
//! sized to the clip. Telemetry comes from `dc_ops::Evaluator` as `dc_core::CursorTracks`.

mod commands;
mod evaluate;
mod path;
mod spring;
mod state;

pub use commands::CursorSet;
pub use evaluate::{Prepared, evaluate};
pub use state::{CursorSettings, CursorState, SIZE_MAX, SIZE_MIN, Smoothing};

use dc_core::{Effect, EffectInfo, OpInfo};

pub struct Cursor;

impl Effect for Cursor {
    const ID: &'static str = "cursor";
    const TITLE: &'static str = "Курсор";
    type State = CursorState;
}

pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<CursorSet>()]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Cursor>(Vec::new())
}

#[cfg(test)]
mod test_util;
#[cfg(test)]
mod tests;
