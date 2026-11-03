//! The timeline of the editor.

use dc_core::OutputTime;

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

/// Title of the timeline window: «Таймлайн — Демо-запись».
pub fn window_title(project: &str) -> String {
    format!("Таймлайн — {project}")
}

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

/// «+ Дорожка ▾» menu and the rows it adds (7.10).
pub const ADD_ANNOTATIONS_ROW: &str = "Дорожка аннотаций";
pub const ADD_AUDIO_ROW: &str = "Аудиодорожка";
pub const MUTE: &str = "Выключить звук";
pub const UNMUTE: &str = "Включить звук";
/// Hint of an annotation row header: dragging it changes the layer order.
pub const LAYER_HINT: &str = "Перетащите, чтобы поднять или опустить слой";
/// The «+» of the dashed ghost of quick adding.
pub const GHOST: &str = "+";
pub const PICK_SOUND: &str = "Добавить звук";
pub const SOUND_FILES: &str = "Звук";
pub const REMOVE_CANCEL: &str = "Отмена";
pub const REMOVE_CONFIRM: &str = "Удалить дорожку";

/// «Удалить «Аннотации 1»?».
pub fn remove_title(name: &str) -> String {
    format!("Удалить «{name}»?")
}

/// «На дорожке 2 аннотации, они удалятся вместе с ней. Вернуть — Ctrl+Z.» (`undo` — the user's
/// undo keys, empty when unassigned).
pub fn remove_annotations(count: usize, undo: &str) -> String {
    let what = plural(count, "аннотация", "аннотации", "аннотаций");
    removed(count, what, "она", undo)
}

/// «На дорожке 1 файл, он удалится вместе с ней. Вернуть — Ctrl+Z.».
pub fn remove_sound(count: usize, undo: &str) -> String {
    let what = plural(count, "файл", "файла", "файлов");
    removed(count, what, "он", undo)
}

/// `it` is the pronoun of one object.
fn removed(count: usize, what: &str, it: &str, undo: &str) -> String {
    let text = if count == 1 {
        format!("На дорожке {count} {what}, {it} удалится вместе с ней.")
    } else {
        format!("На дорожке {count} {what}, они удалятся вместе с ней.")
    };
    match undo {
        "" => text,
        undo => format!("{text} Вернуть — {undo}."),
    }
}

/// The Russian form of `count`: «1 файл», «2 файла», «5 файлов».
pub(crate) fn plural<'a>(count: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let (tens, ones) = (count % 100, count % 10);
    match ones {
        1 if tens != 11 => one,
        2..=4 if !(12..=14).contains(&tens) => few,
        _ => many,
    }
}

/// «Ключ 0:30.00»: the name of a key marker on an annotation (7.12).
pub fn key_marker(at: &str) -> String {
    format!("Ключ {at}")
}

/// «Клик 0:21.40»: the name of a click dot on the «Курсор» row (7.15).
pub fn click_dot(at: &str) -> String {
    format!("Клик {at}")
}

/// «Ctrl+S», «Ctrl+Shift+P ×2»: a shortcut chip on the «Клавиши» row, `count` presses in a row.
pub fn shortcut(name: &str, count: usize) -> String {
    match count {
        0 | 1 => name.to_owned(),
        n => format!("{name} ×{n}"),
    }
}

/// «Ключ 2 · 0:33.40»: the title of a key marker's tooltip (7.25, mockup 2e).
pub fn key_tip(number: usize, at: &str) -> String {
    format!("Ключ {number} · {at}")
}

/// «по сравнению с ключом 1:»: what the lines of the tooltip compare the key with.
pub fn key_since(number: usize) -> String {
    format!("по сравнению с ключом {number}:")
}

pub const KEY_SINCE_BASE: &str = "по сравнению с аннотацией без ключей:";
pub const KEY_SAME: &str = "Без изменений";
pub const KEY_NONE: &str = "нет";
pub const KEY_ON: &str = "вкл";
pub const KEY_OFF: &str = "выкл";
pub const KEY_POSITION: &str = "Позиция";
pub const KEY_SIZE: &str = "Размер";
pub const KEY_ROTATION: &str = "Поворот";

/// «Позиция: 120, 80 → 120, 140»: a line of the tooltip.
pub fn key_change(name: &str, from: &str, to: &str) -> String {
    format!("{name}: {from} → {to}")
}

/// «Штрихи: изменено»: a list or a group of fields changed.
pub fn key_changed(name: &str) -> String {
    format!("{name}: изменено")
}

/// The name of field `name` of an annotation kind (the bodies of `dc-fx-*`) in the tooltip.
pub fn key_field(name: &str) -> &str {
    match name {
        "text" => "Текст",
        "font" => "Шрифт",
        "size" => "Размер шрифта",
        "bold" => "Жирный",
        "italic" => "Курсив",
        "underline" => "Подчёркнутый",
        "align" => "Выравнивание",
        "color" => "Цвет",
        "background" => "Цвет плашки",
        "radius" => "Скругление",
        "figure" => "Фигура",
        "from" => "Начало",
        "to" => "Конец",
        "width" => "Толщина",
        "outline" => "Обводка",
        "fill" => "Заливка",
        "gradient" => "Градиент",
        "colors" => "Цвета градиента",
        "angle" => "Направление",
        "cycle" => "Круг градиента",
        "glow" => "Свечение",
        "opacity" => "Прозрачность",
        "picture" => "Картинка",
        "strength" => "Сила",
        "tint" => "Тонировка",
        "strokes" => "Штрихи",
        other => other,
    }
}

/// «0:33.40»: minutes, seconds and hundredths; hours in front from one hour on.
pub fn timecode(t: OutputTime) -> String {
    let hundredths = t.micros().max(0) / 10_000;
    let (seconds, cc) = (hundredths / 100, hundredths % 100);
    format!("{}.{cc:02}", ruler_label(seconds))
}

/// «1:10» of a ruler tick at `seconds`.
pub fn ruler_label(seconds: i64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
