//! Pressed-keys overlay: the shortcuts pressed during the recordings on a plate over the frame,
//! fast repeats counted. Telemetry comes from `dc_ops::Evaluator` as `dc_core::KeyTracks`.

mod commands;
mod evaluate;
mod prepared;
mod state;

pub use commands::KeysSet;
pub use evaluate::evaluate;
pub use prepared::Prepared;
pub use state::{
    DURATION_MAX, DURATION_MIN, KeysSettings, KeysState, Position, SIZE_MAX, SIZE_MIN,
};

use dc_core::{Effect, EffectInfo, OpInfo};

pub struct Keys;

impl Effect for Keys {
    const ID: &'static str = "keys";
    const TITLE: &'static str = "Клавиши";
    type State = KeysState;
}

pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<KeysSet>()]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Keys>(Vec::new())
}

#[cfg(test)]
mod test_util;
#[cfg(test)]
mod tests;
