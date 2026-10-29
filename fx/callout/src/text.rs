//! Rules of a callout text: one line, at most `TEXT_MAX` characters.

use dc_core::CommandError;

use crate::model::TEXT_MAX;

/// `text` without the spaces around it, if it is a non-empty single line within the limit.
pub(crate) fn check_text(text: &str) -> Result<&str, CommandError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CommandError::new(
            "текст облачка пустой: напиши хотя бы один символ",
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(CommandError::new(
            "текст облачка — одна строка: убери переводы строк и табуляцию",
        ));
    }
    let length = text.chars().count();
    if length > TEXT_MAX {
        return Err(CommandError::new(format!(
            "текст облачка длиннее {TEXT_MAX} символов ({length}): сократи его"
        )));
    }
    Ok(text)
}
