//! The recordings library of the editor (mockup 6 «Записи»).

pub const TITLE: &str = "Записи";
pub const OPEN_FOLDER: &str = "Открыть папку";
pub const IMPORT: &str = "Импорт…";
pub const PICK_VIDEOS: &str = "Видео для библиотеки";
pub const VIDEOS: &str = "Видео";
pub const SEARCH: &str = "Поиск";
pub const TO_TIMELINE: &str = "На таймлайн";
pub const TRASH: &str = "В корзину";
pub const UNDO: &str = "Отменить · Ctrl+Z";
pub const EMPTY: &str = "Записей пока нет: они появятся здесь после записи или импорта видео.";
pub const NOT_FOUND: &str = "Ничего не найдено.";
pub const IMPORTED: &str = "импорт";

/// «Выбрано 2».
pub fn picked(count: usize) -> String {
    format!("Выбрано {count}")
}

/// «На таймлайн: recording-3» — the + of a tile.
pub fn add_one(name: &str) -> String {
    format!("{TO_TIMELINE}: {name}")
}

/// «2 записи в корзине».
pub fn trashed(count: usize) -> String {
    let word = match (count % 10, count % 100) {
        (1, n) if n != 11 => "запись",
        (2..=4, n) if !(12..=14).contains(&n) => "записи",
        _ => "записей",
    };
    format!("{count} {word} в корзине")
}

/// «2560 × 1440» or «импорт · 2560 × 1440»: what a tile is under its name.
pub fn details(imported: bool, width: u32, height: u32) -> String {
    match imported {
        true => format!("{IMPORTED} · {width} × {height}"),
        false => format!("{width} × {height}"),
    }
}

/// The library could not move the recordings to the Recycle Bin.
pub fn trash_failed(reason: &str) -> String {
    format!(
        "Не удалось переместить в корзину: {reason}. Закройте программы, которые держат файлы записи, и попробуйте снова."
    )
}

/// The library could not bring the recordings back from the Recycle Bin.
pub fn restore_failed(reason: &str) -> String {
    format!(
        "Не удалось вернуть из корзины: {reason}. Восстановите записи из корзины Windows вручную."
    )
}

/// «recording-3.devcast» is not in the Recycle Bin any more.
pub fn not_in_bin(name: &str) -> String {
    format!("«{name}» нет в корзине: её уже очистили или восстановили")
}
