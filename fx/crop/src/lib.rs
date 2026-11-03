//! Output aspect and the command that crops a clip (the crop itself is `Clip::crop` in dc-core).
//! The size of the output frame (`canvas`); clips are fitted into it by their video layer. The
//! «Кроп и формат» dialog (`dialog`), which the editor opens.

mod canvas;
mod commands;
pub mod dialog;
mod dialog_geom;
mod dialog_view;
pub mod stage;
mod state;

pub use canvas::canvas;
pub use commands::{CropClip, SetAspect};
pub use state::{Aspect, CropSettings, CropState};

use dc_core::{Effect, EffectInfo, OpInfo, SettingsArgs};

pub struct Crop;

impl Effect for Crop {
    const ID: &'static str = "crop";
    const TITLE: &'static str = "Кроп";
    type State = CropState;

    fn settings_args(state: &CropState) -> Result<Vec<SettingsArgs>, serde_json::Error> {
        let aspect = SetAspect {
            aspect: state.settings.aspect,
        };
        Ok(vec![SettingsArgs::of(&aspect)?])
    }
}

pub fn ops() -> Vec<OpInfo> {
    vec![OpInfo::of::<CropClip>(), OpInfo::of::<SetAspect>()]
}

pub fn effect() -> EffectInfo {
    EffectInfo::of::<Crop>(Vec::new())
}

#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod tests;
