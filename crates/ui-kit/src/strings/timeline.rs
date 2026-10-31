//! The timeline of the editor.

pub const ADD_TRACK: &str = "Дорожка";
pub const MAGNET: &str = "Привязка";
pub const SUGGEST_ZOOMS: &str = "Подобрать зумы";
pub const MAKE_CAPTIONS: &str = "Создать субтитры";
pub const SCALE: &str = "Масштаб времени";
pub const FIT: &str = "Весь проект";
pub const RULER: &str = "Шкала времени";
pub const PLAYHEAD: &str = "Указатель";
pub const NAVIGATION: &str = "Прокрутка таймлайна";
pub const DETACH: &str = "Таймлайн в отдельном окне";
pub const ADD_AUDIO: &str = "Аудиодорожка";
pub const CAPTIONS: &str = "Субтитры";
pub const CLIPS: &str = "Клипы";
pub const ZOOMS: &str = "Зумы";
pub const CURSOR: &str = "Курсор";
pub const KEYS: &str = "Клавиши";
pub const MIC: &str = "Микрофон";
pub const SYSTEM: &str = "Системный звук";
pub const SHOWN: &str = "Показывается";
pub const HIDDEN: &str = "Скрыто";
pub const SHOW: &str = "Показать";
pub const HIDE: &str = "Скрыть";
pub const CLIP_CROP: &str = "Кроп клипа";
pub const REMOVE_TRACK: &str = "Убрать дорожку";
pub const MUTED: &str = "Без звука";
pub const SOUND: &str = "Звук";
pub const CALLOUT_ON: &str = "вкл";
pub const CALLOUT_OFF: &str = "выкл";
pub const TEXT: &str = "Текст";
pub const SHAPE: &str = "Фигура";
pub const IMAGE: &str = "Картинка";
pub const DRAWING: &str = "Рисунок";
pub const BLUR: &str = "Размытие";
/// Fixed zooms look at their point instead of following the cursor.
pub const FIXED: &str = "точка";
pub const SPEED_MENU: &str = "Скорость участка…";
pub const SPEED_TITLE: &str = "Скорость участка";
pub const OWN_SPEED: &str = "Своя";
pub const APPLY: &str = "Применить";
/// After a speed in its field.
pub const TIMES: &str = "×";
/// Sped up or slowed down sound keeps its pitch (dc-audio tempo).
pub const PITCH_KEPT: &str = "Высота голоса сохраняется.";

/// «Участок 0:13.200 – 0:22.800 · 9,6 с».
pub fn range(start: &str, end: &str, secs: f64) -> String {
    format!("Участок {start} – {end} · {}", seconds(secs))
}

/// «16,0 с → 4,0 с»: the range now and at the new speed.
pub fn speed_change(before: f64, after: f64) -> String {
    format!("{} → {}", seconds(before), seconds(after))
}

/// «9,6 с».
pub fn seconds(secs: f64) -> String {
    format!("{secs:.1} с").replace('.', ",")
}

/// «Скорость 4×»: the band of a speed range.
pub fn speed_band(factor: f64) -> String {
    format!("Скорость {}", times(factor))
}

/// «Аннотации 2».
pub fn annotations(number: u32) -> String {
    format!("Аннотации {number}")
}

/// «Аудио 1».
pub fn audio(number: u32) -> String {
    format!("Аудио {number}")
}

/// «Клип 3».
pub fn clip(number: u32) -> String {
    format!("Клип {number}")
}

/// «1,5×»: a scale or speed with a decimal comma.
pub fn times(factor: f64) -> String {
    let text = format!("{factor:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{}×", text.replace('.', ","))
}

/// «1,8× · точка».
pub fn fixed_zoom(scale: f64) -> String {
    format!("{} · {FIXED}", times(scale))
}

/// «0:52.0 · 2560 × 1440».
pub fn clip_facts(length: &str, width: u32, height: u32) -> String {
    format!("{length} · {width} × {height}")
}

/// «Выбрано: 4 · Delete удалить · Ctrl+C копировать»: several picked regions (mockup 2a);
/// `delete` and `copy` are the user's keys, empty when unassigned.
pub fn picked(count: usize, delete: &str, copy: &str) -> String {
    let mut text = format!("Выбрано: {count}");
    if !delete.is_empty() {
        text.push_str(&format!(" · {delete} удалить"));
    }
    if !copy.is_empty() {
        text.push_str(&format!(" · {copy} копировать"));
    }
    text
}

/// «0:32.40 – 0:36.80»: where a dragged region is (mockup 2b).
pub fn span(start: &str, end: &str) -> String {
    format!("{start} – {end}")
}
