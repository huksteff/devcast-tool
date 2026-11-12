//! Cursor callout bubble: events «on», «text», «off» on the timeline of each recording, the typed
//! text, the look of the bubble and the inspector panel of an event (`panel`).

mod commands;
mod evaluate;
pub mod lane;
pub mod look;
mod model;
pub mod panel;
mod resolve;
mod style;
mod text;

pub use commands::{ActionName, CalloutAdd, CalloutRm, CalloutSet};
pub use evaluate::evaluate;
pub use model::{
    Action, Callout, CalloutEvent, CalloutSettings, CalloutState, EventId, EventKind, GLOW_MAX,
    SIZE_MAX, SIZE_MIN, TEXT_MAX,
};
pub use resolve::text_at;
pub use style::CalloutStyle;

use dc_core::{EffectInfo, IdKind, OpInfo};

/// All callout commands, in the order `devcast help` lists them.
pub fn ops() -> Vec<OpInfo> {
    vec![
        OpInfo::of::<CalloutAdd>(),
        OpInfo::of::<CalloutSet>(),
        OpInfo::of::<CalloutRm>(),
        OpInfo::of::<CalloutStyle>(),
    ]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Callout>(vec![EventKind::PREFIX])
}

#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod resolve_tests;
#[cfg(test)]
mod tests;
