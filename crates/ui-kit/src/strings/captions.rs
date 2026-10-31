//! The captions panel of the editor (mockup 3b): recognition, the cue list, the look.

pub const SHOW: &str = "Показывать субтитры";
pub const RECOGNITION: &str = "Распознавание";
pub const LANGUAGE: &str = "Язык";
pub const AUTO: &str = "Авто";
pub const RUSSIAN: &str = "Русский";
pub const ENGLISH: &str = "Английский";
pub const TRACKS: &str = "Дорожки";
pub const MIC: &str = "Микрофон";
pub const SYSTEM: &str = "Системный звук";
pub const BOTH: &str = "Микрофон и система";
pub const REMAKE: &str = "Пересоздать";
pub const CLEAR: &str = "Очистить все";
pub const CANCEL: &str = "Остановить";
pub const CUES: &str = "Реплики";
pub const EMPTY: &str = "Субтитров ещё нет";
pub const EMPTY_READY: &str = "Речь распознаётся на этом компьютере, без интернета.";

pub const CUE_TEXT: &str = "Текст реплики";
pub const CUE_START: &str = "Начало реплики";
pub const CUE_END: &str = "Конец реплики";
pub const SPLIT: &str = "Разделить";
pub const SPLIT_HINT: &str = "Разделить в паузе у указателя";
pub const MERGE: &str = "Объединить";
pub const MERGE_HINT: &str = "Объединить со следующей репликой";
pub const CUE_NOTE: &str =
    "Enter — применить. Исправленные слова получают новое время, остальные — своё.";

pub const STYLE: &str = "Стиль";
pub const FONT: &str = "Шрифт";
pub const SIZE: &str = "Размер";
pub const WORDS: &str = "Цвета слов";
pub const COLOR: &str = "Цвет сказанных слов";
pub const SHADOW: &str = "Тень";
pub const HIGHLIGHT: &str = "Цвет текущего слова";
pub const INACTIVE: &str = "Цвет несказанных слов";
pub const ANIMATION: &str = "Анимация";
pub const NO_ANIMATION: &str = "Нет";
pub const FADE: &str = "Fade";
pub const RISE: &str = "Rise";
pub const POP: &str = "Pop";
pub const ROWS: &str = "Строк";
pub const BOTTOM: &str = "Отступ снизу";
pub const WIDTH: &str = "Ширина";
pub const PLATE: &str = "Плашка";
pub const RADIUS: &str = "Скругление";
/// Unit after a font size in px.
pub const PX: &str = " px";

pub const NO_ANSWER: &str =
    "распознавание остановилось без ответа: попробуй ещё раз; если повторится — смотри журнал";
pub const NO_MODELS: &str =
    "папка моделей не найдена: проверь папку настроек %APPDATA%\\DevCast и перезапусти программу";

/// «3 реплики», «реплик нет»: the subtitle of the panel.
pub fn cues_count(count: usize) -> String {
    match count {
        0 => "реплик нет".to_owned(),
        n => format!(
            "{n} {}",
            super::timeline::plural(n, "реплика", "реплики", "реплик")
        ),
    }
}

/// «Распознаю… 64 % · модель «Быстрая»».
pub fn recognizing(percent: Option<u32>, model: &str) -> String {
    match percent {
        Some(p) => format!("Распознаю… {p} % · модель «{model}»"),
        None => format!("Распознаю… · модель «{model}»"),
    }
}

/// «Скачиваю модель «Быстрая»… 12 %».
pub fn downloading(percent: Option<u32>, model: &str) -> String {
    match percent {
        Some(p) => format!("Скачиваю модель «{model}»… {p} %"),
        None => format!("Скачиваю модель «{model}»…"),
    }
}

/// «Модель «Быстрая» (≈ 480 МБ) скачается один раз».
pub fn model_missing(model: &str, size: &str) -> String {
    format!("Модель «{model}» ({size}) скачается один раз")
}

/// The size of a model file to download.
pub const FAST_SIZE: &str = "≈ 480 МБ";
pub const ACCURATE_SIZE: &str = "≈ 1,6 ГБ";

/// Why a recognition did not start.
pub fn failed_start(error: &dyn std::fmt::Display) -> String {
    format!("не удалось запустить распознавание: {error}. Перезапусти программу")
}

/// «0:09.00 Открываем страницу логина»: the accessible name of a line of the cue list (the
/// region of the cue on the timeline is named by its text alone).
pub fn cue_line(at: &str, said: &str) -> String {
    format!("{at} {said}")
}
