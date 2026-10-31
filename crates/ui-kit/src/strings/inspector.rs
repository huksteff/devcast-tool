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

pub const ZOOM_MODE: &str = "Режим";
pub const ZOOM_FOLLOW: &str = "За курсором";
pub const ZOOM_FIXED: &str = "Точка";
pub const ZOOM_FOLLOW_NOTE: &str = "Камера идёт за курсором, начиная с точки фокуса.";
pub const ZOOM_FIXED_NOTE: &str = "Камера стоит на точке фокуса.";
pub const ZOOM_SCALE: &str = "Масштаб";
pub const ZOOM_X: &str = "Фокус по X";
pub const ZOOM_Y: &str = "Фокус по Y";
pub const ZOOM_MOTION: &str = "Движение";
pub const ZOOM_FOCUSED: &str = "Focused";
pub const ZOOM_SMOOTH: &str = "Smooth";
pub const ZOOM_CONNECT: &str = "Склеить с соседним";
pub const ZOOM_AUTO: &str = "Автозумы после записи";
pub const ZOOM_SMOOTHNESS: &str = "Плавность камеры";
pub const ZOOM_IN: &str = "Вход";
pub const ZOOM_OUT: &str = "Выход";
pub const ZOOM_BLUR: &str = "Motion blur";
pub const ZOOM_SHARED: &str = "Движение, склейка и тонкая настройка общие для всех зумов проекта.";
pub const ZOOM_SUGGEST: &str = "Подобрать зумы по кликам";
/// Unit after a percentage in a number field.
pub const PERCENT: &str = " %";

pub const CURSOR_PANEL: &str = "Курсор и клики";
pub const CURSOR_GROUP: &str = "Курсор";
pub const CURSOR_SHOW: &str = "Показать";
pub const CURSOR_SET: &str = "Набор";
pub const CURSOR_SYSTEM: &str = "Системный";
pub const CURSOR_OWN: &str = "Своя картинка";
pub const CURSOR_OWN_PICK: &str = "Выбрать файл…";
/// Why «Своя картинка» is off: the owner froze own cursors (QUESTIONS.md, 2026-10-29).
pub const CURSOR_OWN_FROZEN: &str =
    "Свои курсоры пока заморожены: картинку своего курсора добавим отдельной задачей.";
pub const CURSOR_SIZE: &str = "Размер";
pub const CURSOR_MOTION: &str = "Движение";
pub const CURSOR_EXACT: &str = "Точно";
pub const CURSOR_BOUNCE: &str = "Отскок";
pub const CURSOR_BOUNCE_TIME: &str = "Длина отскока";
pub const CURSOR_SWAY: &str = "Покачивание";
pub const CURSOR_BLUR: &str = "Motion blur";
pub const CURSOR_LOOP: &str = "Зациклить";
pub const CURSOR_LOOP_NOTE: &str = "последние 0,67 с";

pub const CLICK_GROUP: &str = "Эффект клика";
pub const CLICK_OFF: &str = "Выкл";
pub const CLICK_RIPPLE: &str = "Ripple";
pub const CLICK_SPOTLIGHT: &str = "Spotlight";
pub const CLICK_ECHO: &str = "Echo";
pub const CLICK_FLASH: &str = "Вспышка";
pub const CLICK_COLOR: &str = "Цвет";
pub const BLUE: &str = "Синий";
pub const RED: &str = "Красный";
pub const AMBER: &str = "Жёлтый";
pub const GREEN: &str = "Зелёный";
pub const PURPLE: &str = "Фиолетовый";
pub const OWN_COLOR: &str = "Свой цвет";
pub const CLICK_SIZE: &str = "Размер клика";
pub const CLICK_OPACITY: &str = "Прозрачность клика";
pub const CLICK_DURATION: &str = "Длительность клика";

/// The panel of one click picked on the «Курсор» row.
pub const CLICK: &str = "Клик";
pub const CLICK_ONE: &str = "Эффект этого клика";
pub const CLICK_ONE_NOTE: &str =
    "Что у клика не меняли, берётся из общего эффекта кликов на вкладке «Курсор».";
pub const LEFT_BUTTON: &str = "левая кнопка";
pub const RIGHT_BUTTON: &str = "правая кнопка";
pub const MIDDLE_BUTTON: &str = "средняя кнопка";
pub const BACK_BUTTON: &str = "кнопка «назад»";
pub const FORWARD_BUTTON: &str = "кнопка «вперёд»";

/// «0:21.40 · левая кнопка», «0:21.40 · левая кнопка, двойной»: the subtitle of a click.
pub fn click_about(at: &str, button: &str, double: bool) -> String {
    match double {
        true => format!("{at} · {button}, двойной"),
        false => format!("{at} · {button}"),
    }
}
