//! Every Russian string of the interface (SCOPE: Russian only), from the approved mockups
//! (docs/design/recording-windows.html). Windows and widgets take their text from here; texts
//! of CLI commands and errors live next to them (docs/QUESTIONS.md). `tests/strings.rs` draws
//! every literal of this file, so a letter missing in the fonts fails a test.

/// Widgets of the kit.
pub mod kit {
    pub const COPY: &str = "Копировать";
    pub const CLOSE: &str = "Закрыть";
    pub const ADD_COLOR: &str = "Добавить цвет";
    pub const REMOVE_COLOR: &str = "Убрать цвет";
    pub const COLOR: &str = "Цвет";
    pub const HEX_HINT: &str = "#RRGGBB";
    pub const COPIED: &str = "Скопировано";
}

/// The HUD bar and its popovers.
pub mod hud {
    pub const GRIP: &str = "Перетащить панель";
    pub const SOURCE: &str = "Источник записи";
    pub const MIC: &str = "Микрофон";
    pub const NO_MIC: &str = "Без микрофона";
    pub const SYSTEM_AUDIO: &str = "Системный звук";
    pub const SOUND: &str = "Звук";
    pub const FPS: &str = "Частота кадров";
    pub const COUNTDOWN: &str = "Обратный отсчёт";
    pub const COUNTDOWN_NONE: &str = "Нет";
    pub const RECORD: &str = "Запись";
    pub const MORE: &str = "Ещё";
    pub const HIDE_FROM_RECORDING: &str = "Скрывать HUD из записи";
    pub const OPEN_PROJECT: &str = "Открыть проект…";
    pub const SETTINGS: &str = "Настройки";
    pub const TO_TRAY: &str = "Свернуть в трей";
    pub const QUIT: &str = "Выход";
    pub const STOP: &str = "Остановить запись";
    pub const REC: &str = "REC";
    pub const PAUSED: &str = "ПАУЗА";
    pub const PAUSE: &str = "Пауза";
    pub const RESUME: &str = "Продолжить";
    pub const CANCEL: &str = "Отменить запись";
    pub const HIDE: &str = "Скрыть панель";
    pub const CANCEL_COUNTDOWN: &str = "Отменить";
    pub const CANCEL_TITLE: &str = "Отменить запись?";
    pub const KEEP_RECORDING: &str = "Продолжить запись";
    pub const DELETE_RECORDING: &str = "Удалить запись";
    pub const PREPARING: &str = "Подготовка записи";
    pub const PREPARING_HINT: &str = "Редактор откроется через пару секунд";
    pub const PREPARE_FAILED: &str = "Запись не подготовлена";
    pub const OPEN_FOLDER: &str = "Открыть папку записи";

    /// Countdown choice: «3 с».
    pub fn seconds(n: u32) -> String {
        format!("{n} с")
    }

    pub fn cancel_text(recorded: &str) -> String {
        format!("Запись на паузе. Записанные {recorded} будут удалены. Это нельзя вернуть.")
    }
}

/// The source picker.
pub mod picker {
    pub const TITLE: &str = "Источник записи";
    pub const SCREENS: &str = "Экраны";
    pub const WINDOWS: &str = "Окна";
    pub const AREA: &str = "Область…";
    pub const NO_AREA: &str = "выделить на экране";
    pub const EMPTY: &str = "Источники изображения не найдены";
    pub const REFRESH: &str = "Обновить";
    pub const REFRESHING: &str = "Обновляю…";
    pub const PRIMARY: &str = "основной";

    /// «Экран 1».
    pub fn screen(number: u32) -> String {
        format!("Экран {number}")
    }

    /// «последняя: 1280 × 720».
    pub fn last_area(width: u32, height: u32) -> String {
        format!("последняя: {width} × {height}")
    }
}

/// Selecting an area over the screen.
pub mod area {
    pub const HINT: &str = "Нажмите и тяните, чтобы выделить область";
    pub const HINT_KEYS: &str = "Enter — начать, Esc — отмена";
    pub const MOVE: &str = "Перетащите, чтобы сдвинуть";
    pub const FREE: &str = "Свободно";
    pub const WIDE: &str = "16:9";
    pub const FULL_HD: &str = "1080p";
    pub const CANCEL: &str = "Отмена";
    pub const START: &str = "Начать запись";

    /// «Слишком мало: 120 × 80».
    pub fn too_small(width: u32, height: u32) -> String {
        format!("Слишком мало: {width} × {height}")
    }
}

/// The countdown window.
pub mod countdown {
    pub const CANCEL_HINT: &str = "Клик или Esc — отмена";
}

/// The settings window.
pub mod settings {
    pub const TITLE: &str = "Настройки — DevCast";
    pub const RECORDING: &str = "Запись";
    pub const DEFAULTS: &str = "По умолчанию";
    pub const HOTKEYS: &str = "Горячие клавиши";
    pub const CAPTIONS: &str = "Субтитры";
    pub const UPDATES: &str = "Обновления";
    pub const ABOUT: &str = "О программе";
    pub const FOLDER: &str = "Папка записей и проектов";
    pub const CHOOSE: &str = "Выбрать…";
    pub const HIDE_HINT: &str = "панель не попадает в видео";
    pub const MODEL: &str = "Модель распознавания речи (Whisper, локально)";
    pub const MODEL_FAST: &str = "Быстрая";
    pub const MODEL_FAST_HINT: &str = "small · ≈ 480 МБ · по умолчанию";
    pub const MODEL_ACCURATE: &str = "Точная";
    pub const MODEL_ACCURATE_HINT: &str = "large-v3-turbo · ≈ 1,6 ГБ";
    pub const AUTO_UPDATES: &str = "Проверять автоматически";
    pub const AUTO_UPDATES_HINT: &str = "раз в 6 часов";
    pub const BETA: &str = "Бета-версии";
    pub const CHECK_UPDATES: &str = "Проверить обновления";
    pub const MANUAL_UPDATES: &str = "Обновления пока ставятся вручную";
    pub const DEFAULTS_HINT: &str = "Настройки эффектов для новых проектов меняются в редакторе: «Сохранить текущие как по умолчанию».";

    /// «Версия 2.0.0».
    pub fn version(version: &str) -> String {
        format!("Версия {version}")
    }
}

/// Global hotkeys in the settings.
pub mod hotkeys {
    pub const SEARCH: &str = "Найти команду или сочетание";
    pub const GLOBAL: &str = "Глобальные (работают везде)";
    pub const RESET_ALL: &str = "Сбросить все";
    pub const START_STOP: &str = "Начать / остановить запись";
    pub const PAUSE: &str = "Пауза / продолжить";
    pub const CANCEL: &str = "Отменить запись";
    pub const PRESS: &str = "Нажмите сочетание…";
    pub const NONE: &str = "Нет";
    pub const RESET: &str = "Сбросить";
    pub const TAKEN_BY_SYSTEM: &str =
        "Это сочетание уже занято другой программой или системой. Выберите другое.";
    pub const CHANGE: &str = "Изменить";
    pub const REPLACE: &str = "Заменить";
    pub const CANCEL_CAPTURE: &str = "Отмена";
    pub const HELP: &str = "Нажмите нужные клавиши. Esc — отмена, Delete — убрать сочетание. Нужен Ctrl, Alt или Win. В русской раскладке работает так же.";

    /// «Ctrl+Shift+R уже назначено на «Начать / остановить запись»».
    pub fn taken_by(keys: &str, command: &str) -> String {
        format!("{keys} уже назначено на «{command}»")
    }
}

/// The tray icon and its menu.
pub mod tray {
    pub const TOOLTIP: &str = "DevCast";
    pub const READY: &str = "DevCast — готов к записи";
    pub const SHOW: &str = "Показать панель";
    pub const STOP: &str = "Остановить";
    pub const SETTINGS: &str = "Настройки";
    pub const QUIT: &str = "Выход";
    pub const QUIT_TITLE: &str = "Идёт запись";
    pub const QUIT_TEXT: &str = "Остановить и сохранить запись перед выходом?";
    pub const QUIT_CANCEL: &str = "Отмена";
    pub const QUIT_SAVE: &str = "Сохранить и выйти";

    /// «● Запись 12:47 · Экран 1».
    pub fn recording(time: &str, source: &str) -> String {
        format!("● Запись {time} · {source}")
    }
}
