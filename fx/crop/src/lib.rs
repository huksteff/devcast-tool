//! Per-clip crop and output aspect. Model and commands only; the frame is drawn by the render.

mod commands;
mod state;

pub use commands::{CropClip, SetAspect};
pub use state::{Aspect, CropSettings, CropState, MIN_SIZE};

use dc_core::{Effect, EffectInfo, OpInfo};

pub struct Crop;

impl Effect for Crop {
    const ID: &'static str = "crop";
    const TITLE: &'static str = "Кроп";
    type State = CropState;
}

pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<CropClip>(), OpInfo::of::<SetAspect>()]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Crop>(Vec::new())
}

#[cfg(test)]
mod tests;
