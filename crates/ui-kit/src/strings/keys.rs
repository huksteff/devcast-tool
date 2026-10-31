//! Editor shortcuts: names of the actions and groups, the help window, key caps.

pub const PLAYBACK: &str = "Воспроизведение";
pub const MONTAGE: &str = "Монтаж";
pub const EFFECTS: &str = "Эффекты";
pub const EDIT: &str = "Правка";
pub const PROJECT: &str = "Проект";

pub const PLAY_PAUSE: &str = "Пуск / пауза";
pub const PREV_FRAME: &str = "Кадр назад";
pub const NEXT_FRAME: &str = "Кадр вперёд";
pub const PREV_KEY: &str = "К предыдущему ключу или −5 с";
pub const NEXT_KEY: &str = "К следующему ключу или +5 с";
pub const CUT_AT_PLAYHEAD: &str = "Разрезать у указателя";
pub const SELECT: &str = "Выбор";
pub const BLADE: &str = "Лезвие";
pub const CUT_RANGE: &str = "Вырезать участок";
pub const SNAP: &str = "Привязка вкл/выкл";
pub const ADD_ZOOM: &str = "Добавить зум";
pub const ADD_ANNOTATION: &str = "Добавить аннотацию";
pub const KEYFRAME: &str = "Ключевой кадр";
pub const NEXT_ANNOTATION: &str = "Следующая аннотация";
pub const UNDO: &str = "Отменить";
pub const REDO: &str = "Повторить";
pub const REDO_ALT: &str = "Повторить (2)";
pub const COPY: &str = "Копировать";
pub const CUT: &str = "Вырезать";
pub const PASTE: &str = "Вставить у указателя";
pub const SELECT_ALL: &str = "Выбрать всё";
pub const DELETE: &str = "Удалить выбранное";
pub const NEW_PROJECT: &str = "Новый проект";
pub const OPEN_PROJECT: &str = "Открыть проект";
pub const SAVE: &str = "Сохранить";
pub const SAVE_AS: &str = "Сохранить как";
pub const HELP: &str = "Справка по клавишам";

/// The help window.
pub const TITLE: &str = "Горячие клавиши";
pub const SEARCH: &str = "Найти команду или клавишу";
pub const EDIT_IN_SETTINGS: &str = "Изменить в настройках";
pub const NOT_READY: &str = "Пока не работает: функция ещё не готова";
pub const NOTHING_FOUND: &str = "Ничего не найдено";
pub const FOOTER: &str =
    "Работают и в русской раскладке. Серым — клавиши функций, которые ещё не готовы.";

/// The editor group of the settings.
pub const EDITOR_GROUP: &str = "Редактор";

/// How a key is written on its cap, when it differs from its Latin name.
pub fn cap(name: &str) -> Option<&'static str> {
    Some(match name {
        "Left" => "←",
        "Right" => "→",
        "Up" => "↑",
        "Down" => "↓",
        "Delete" => "Del",
        _ => return None,
    })
}

/// A tooltip with a shortcut: «Выбор · A».
pub fn with_keys(title: &str, keys: &str) -> String {
    format!("{title} · {keys}")
}
