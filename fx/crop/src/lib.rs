//! Output aspect and the command that crops a clip (the crop itself is `Clip::crop` in dc-core).
//! The size of the output frame (`canvas`); clips are fitted into it by their video layer.

mod canvas;
mod commands;
mod state;

pub use canvas::canvas;
pub use commands::{CropClip, SetAspect};
pub use state::{Aspect, CropSettings, CropState};

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
