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
    pub const NO_MICS: &str = "Нет доступных микрофонов";
    pub const SYSTEM_AUDIO: &str = "Системный звук";
    pub const SOUND: &str = "Звук";
    pub const FPS: &str = "Частота кадров";
    pub const TIMING: &str = "Частота кадров и отсчёт";
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
    pub const WINDOW: &str = "Окно";
    pub const AREA: &str = "Область";

    /// «Экран 1 · 2560×1440».
    pub fn sized(name: &str, width: u32, height: u32) -> String {
        format!("{name} · {width}×{height}")
    }

    /// Accessible name of a level indicator: «Уровень: Системный звук».
    pub fn level(source: &str) -> String {
        format!("Уровень: {source}")
    }

    /// Countdown choice: «3 с».
    pub fn seconds(n: u32) -> String {
        format!("{n} с")
    }

    /// Toast after the stop: the recording is there, its auto zooms are not.
    pub fn auto_zooms_failed(error: &str) -> String {
        format!("Автозумы не поставлены: {error}. Добавь их в редакторе: «Подобрать зумы»")
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

    /// Size and DPI scale of a screen: «2560 × 1440 · 150%».
    pub fn screen_size(width: u32, height: u32, scale: u32) -> String {
        format!("{width} × {height} · {scale}%")
    }
}

/// Selecting an area over the screen.
pub mod area {
    pub const HINT: &str = "Нажмите и тяните, чтобы выделить область";
    /// Key caps of the hint, each followed by what it does.
    pub const KEY_START: &str = "Enter";
    pub const KEY_START_HINT: &str = "начать";
    pub const KEY_CANCEL: &str = "Esc";
    pub const KEY_CANCEL_HINT: &str = "отмена";
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

/// The editor window: header, preview, transport, inspector.
pub mod editor {
    pub const LIBRARY: &str = "Записи";
    pub const PROJECTS: &str = "Проекты";
    pub const UNDO: &str = "Отменить";
    pub const REDO: &str = "Повторить";
    pub const UNSAVED: &str = "Есть несохранённые изменения";
    pub const EXTENSION: &str = ".devcast";
    pub const HELP: &str = "Справка";
    pub const SETTINGS: &str = "Настройки";
    pub const EXPORT: &str = "Экспорт";
    pub const CROP: &str = "Кроп и формат";
    pub const PREVIEW: &str = "Превью";
    pub const FIT: &str = "Вписать";
    pub const HALF: &str = "50%";
    pub const ACTUAL: &str = "100%";
    pub const QUALITY_FULL: &str = "Полное";
    pub const QUALITY_FAST: &str = "Быстрое";
    pub const PREVIEW_STOPPED: &str = "Превью остановилось: закрой редактор и открой проект снова";
    /// The output frame in the preview, edited with the pointer (7.21, 7.22).
    pub const PREVIEW_FRAME: &str = "Кадр превью";
    pub const ASPECT_FREE: &str = "Свободный";
    /// The source frame of the first clip, in the preview and the crop dialog.
    pub const ASPECT_SOURCE: &str = "Исходный";
    pub const ASPECT_16_9: &str = "16:9";
    pub const ASPECT_16_10: &str = "16:10";
    pub const ASPECT_4_3: &str = "4:3";
    pub const SELECT: &str = "Выбор";
    pub const BLADE: &str = "Лезвие";
    pub const AUTO_KEYS: &str = "Авто-ключи";
    pub const MARKERS: &str = "Маркеры ключей";
    pub const PREV_KEY: &str = "К предыдущему ключу";
    pub const NEXT_KEY: &str = "К следующему ключу";
    pub const PREV_FRAME: &str = "Кадр назад";
    pub const NEXT_FRAME: &str = "Кадр вперёд";
    pub const PLAY: &str = "Воспроизвести";
    pub const PAUSE: &str = "Пауза";
    pub const SOUND_STOPPED: &str =
        "Звук превью остановился: картинка идёт без звука. Закрой редактор и открой проект снова";
    pub const VOLUME: &str = "Громкость превью";
    pub const ZOOM: &str = "Зум";
    pub const CURSOR: &str = "Курсор и облачко";
    pub const KEYS: &str = "Показ клавиш";
    pub const CAPTIONS: &str = "Субтитры";
    pub const SOUND: &str = "Звук";
    pub const BASIC: &str = "Основное";
    pub const FINE: &str = "Тонкая настройка";
    pub const RESET: &str = "Сбросить";
    /// Asked by a close after the edits could not be written on the previous one.
    pub const CLOSE_UNSAVED_TITLE: &str = "Закрыть без сохранения последних правок?";
    pub const CLOSE_UNSAVED_TEXT: &str =
        "Правки после последней записи на диск не сохранились и пропадут.";
    pub const CLOSE_UNSAVED_CONFIRM: &str = "Закрыть";
    pub const CLOSE_UNSAVED_CANCEL: &str = "Остаться";

    /// Undo with its step: «Отменить: зум 1,50×»; `step` — summary of the step.
    pub fn undo_step(step: &str) -> String {
        format!("{UNDO}: {step}")
    }

    /// Redo with its step: «Повторить: зум 1,50×».
    pub fn redo_step(step: &str) -> String {
        format!("{REDO}: {step}")
    }

    /// The sound of playback failed; `error` says why and what to do.
    pub fn sound_failed(error: &str) -> String {
        format!("Звук превью не играет, картинка идёт без него: {error}")
    }

    /// «recording-1 — DevCast».
    pub fn title(name: &str) -> String {
        format!("{name} — DevCast")
    }

    /// Scale on the zoom frame of the preview: «1,5×».
    pub fn zoom_scale(scale: f64) -> String {
        format!("{scale:.1}×").replace('.', ",")
    }

    /// «16:9 · 1920 × 1080».
    pub fn format(aspect: &str, width: u32, height: u32) -> String {
        format!("{aspect} · {width} × {height}")
    }
}

/// The timeline of the editor.
pub mod timeline;

/// The inspector of the editor.
pub mod inspector;

/// The captions panel of the editor.
pub mod captions;

/// The annotation panels of the inspector.
pub mod annotations;

/// The sound panels of the inspector.
pub mod sound;

/// The crop and output format dialog.
pub mod crop;

/// The recordings library of the editor.
pub mod library;

/// The export dialog and queue of the editor.
pub mod export;

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
    pub const MODEL_READY: &str = "Скачана";
    pub const MODEL_MISSING: &str = "Не скачана";
    pub const DOWNLOAD: &str = "Скачать";
    pub const DOWNLOADING: &str = "Скачивается…";
    pub const AUTO_UPDATES: &str = "Проверять автоматически";
    pub const AUTO_UPDATES_HINT: &str = "раз в 6 часов";
    pub const BETA: &str = "Бета-версии";
    pub const CHECK_UPDATES: &str = "Проверить обновления";
    pub const MANUAL_UPDATES: &str = "Обновления пока ставятся вручную";
    pub const DEFAULTS_HINT: &str = "Настройки эффектов для новых проектов меняются в редакторе: «Сохранить текущие как по умолчанию».";

    /// «Скачивается… 42 %».
    pub fn downloading(percent: u32) -> String {
        format!("{DOWNLOADING} {percent} %")
    }

    /// «Версия 2.0.0».
    pub fn version(version: &str) -> String {
        format!("Версия {version}")
    }
}

/// Editor shortcuts and their help.
pub mod keys;

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
    pub const HELP: &str = "Нажмите нужные клавиши. Esc — отмена, Backspace — убрать сочетание. Глобальным нужен Ctrl или Alt. В русской раскладке работает так же.";

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
