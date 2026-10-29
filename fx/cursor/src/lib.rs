//! The cursor of the recordings: the captured system pictures on a smoothed or exact path,
//! sized to the clip. Telemetry comes from `dc_ops::Evaluator` as `dc_core::CursorTracks`.

mod click_commands;
mod click_state;
mod clicks;
mod commands;
mod effects;
mod evaluate;
mod path;
mod pose;
mod prepared;
mod spring;
mod state;

pub use click_commands::{ClickReset, ClickSet, ClicksSet};
pub use click_state::{
    CLICK_MATCH, CLICK_SCALE_MAX, CLICK_SCALE_MIN, CLICK_TIME_MAX, CLICK_TIME_MIN, ClickChange,
    ClickKind, ClickStyle,
};
pub use commands::CursorSet;
pub use evaluate::evaluate;
pub use prepared::Prepared;
pub use state::{
    BLUR_MAX, BOUNCE_MAX, BOUNCE_TIME_MAX, BOUNCE_TIME_MIN, CursorSettings, CursorState, SIZE_MAX,
    SIZE_MIN, SWAY_MAX, Smoothing,
};

use dc_core::{Effect, EffectInfo, OpInfo};

pub struct Cursor;

impl Effect for Cursor {
    const ID: &'static str = "cursor";
    const TITLE: &'static str = "Курсор";
    type State = CursorState;
}

pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<CursorSet>(),
        OpInfo::of::<ClicksSet>(),
        OpInfo::of::<ClickSet>(),
        OpInfo::of::<ClickReset>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Cursor>(Vec::new())
}

#[cfg(test)]
mod click_tests;
#[cfg(test)]
mod test_util;
#[cfg(test)]
mod tests;
