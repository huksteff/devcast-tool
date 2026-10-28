//! All ops and effects of DevCast. A new effect crate adds its `ops()` and `effect()` here.

use dc_ops::Registry;

use crate::error::CliError;

pub fn registry() -> Result<Registry, CliError> {
    Registry::new(Vec::new(), Vec::new()).map_err(|e| {
        CliError::Failed(format!(
            "внутренняя ошибка: реестр команд собран неверно ({e}). Это баг, сообщи о нём"
        ))
    })
}
