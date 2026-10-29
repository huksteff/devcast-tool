//! All ops and effects of DevCast. A new effect crate adds its `ops()` and `effect()` here.

use dc_ops::Registry;

use crate::error::CliError;

pub fn registry() -> Result<Registry, CliError> {
    let ops = [
        dc_fx_cut::ops(),
        dc_fx_crop::ops(),
        dc_fx_zoom::ops(),
        dc_fx_cursor::ops(),
        dc_fx_callout::ops(),
        dc_fx_captions::ops(),
    ]
    .concat();
    let effects = vec![
        dc_fx_crop::effect(),
        dc_fx_zoom::effect(),
        dc_fx_cursor::effect(),
        dc_fx_callout::effect(),
        dc_fx_captions::effect(),
    ];
    Registry::new(ops, effects).map_err(|e| {
        CliError::Failed(format!(
            "внутренняя ошибка: реестр команд собран неверно ({e}). Это баг, сообщи о нём"
        ))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_op_follows_the_rules() {
        let registry = super::registry().unwrap();
        assert!(registry.ops().len() >= 10);
    }
}
