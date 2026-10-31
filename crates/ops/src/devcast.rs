//! All ops and effects of DevCast: the one registry of the CLI and the editor, so the same command
//! gives the same project in both. A new effect crate adds its `ops()` and `effect()` here.

use crate::error::OpsError;
use crate::registry::Registry;

impl Registry {
    pub fn devcast() -> Result<Self, OpsError> {
        let ops = [
            dc_fx_cut::ops(),
            dc_fx_crop::ops(),
            dc_fx_zoom::ops(),
            dc_fx_cursor::ops(),
            dc_fx_callout::ops(),
            dc_fx_keys::ops(),
            dc_fx_captions::ops(),
            dc_fx_annotations::ops(),
            dc_fx_text::ops(),
            dc_fx_shape::ops(),
            dc_fx_drawing::ops(),
            dc_fx_blur::ops(),
            dc_fx_audio_tracks::ops(),
        ]
        .concat();
        let effects = vec![
            dc_fx_crop::effect(),
            dc_fx_zoom::effect(),
            dc_fx_cursor::effect(),
            dc_fx_callout::effect(),
            dc_fx_keys::effect(),
            dc_fx_captions::effect(),
            dc_fx_annotations::effect(),
            dc_fx_audio_tracks::effect(),
        ];
        Self::new(ops, effects).map_err(|e| {
            OpsError::Failed(format!(
                "внутренняя ошибка: реестр команд собран неверно ({e}). Это баг, сообщи о нём"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_op_follows_the_rules() {
        let registry = super::Registry::devcast().unwrap();
        assert!(registry.ops().len() >= 10);
    }
}
