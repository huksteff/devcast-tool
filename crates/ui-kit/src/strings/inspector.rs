//! The inspector of the editor: the panel frame, the panels of selected objects, «Показ клавиш».

/// Subtitle of a panel of settings that apply to the whole project.
pub const WHOLE_PROJECT: &str = "для всего проекта";
pub const DELETE: &str = "Удалить";
pub const CUE: &str = "Реплика";
pub const CALLOUT: &str = "Облачко";
pub const SOUND: &str = "Звук";

pub const KEYS_SHOW: &str = "Показывать";
pub const KEYS_WHERE: &str = "Где";
pub const KEYS_SIZE: &str = "Размер";
pub const KEYS_COLORS: &str = "Буквы · плашка";
pub const KEYS_LETTERS: &str = "Цвет букв";
pub const KEYS_PLATE: &str = "Цвет плашки";
pub const KEYS_HOLD: &str = "Держать";
pub const KEYS_NOTE: &str = "Пишутся только сочетания с модификаторами и спецклавиши. Обычный ввод текста не записывается никогда.";
/// Unit after seconds in a number field.
pub const SECONDS: &str = " с";
/// Unit after a factor in a number field.
pub const TIMES: &str = "×";

pub const TOP_LEFT: &str = "Вверху слева";
pub const TOP: &str = "Вверху";
pub const TOP_RIGHT: &str = "Вверху справа";
pub const BOTTOM_LEFT: &str = "Внизу слева";
pub const BOTTOM: &str = "Внизу";
pub const BOTTOM_RIGHT: &str = "Внизу справа";

/// «Параметры появятся в задаче 7.14»: the body of a panel whose task has not landed.
pub fn later(task: &str) -> String {
    format!("Параметры появятся в задаче {task}")
}

/// «Зум — панель появится в задаче 7.14»: the tooltip of a tab that is off until its task.
pub fn tab_later(title: &str, task: &str) -> String {
    format!("{title} — панель появится в задаче {task}")
}

/// «Аннотации 1 · 0:29.00 – 0:39.00»: an object on a row of several.
pub fn on_row(row: &str, span: &str) -> String {
    format!("{row} · {span}")
}

/// «0:41.00 – 0:48.00 · 7,0 с»: an object and its length.
pub fn with_length(span: &str, length: &str) -> String {
    format!("{span} · {length}")
}

/// «Сбросить «Основное»»: the reset button of a group, named by the group.
pub fn reset_group(group: &str) -> String {
    format!("Сбросить «{group}»")
}
