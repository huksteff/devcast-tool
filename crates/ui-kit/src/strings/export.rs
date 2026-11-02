//! The export dialog and the export queue of the editor.

pub const TITLE: &str = "Экспорт";
pub const FORMAT: &str = "Формат";
pub const MP4: &str = "MP4";
pub const GIF: &str = "GIF";
pub const RESOLUTION: &str = "Разрешение";
pub const SIZE: &str = "Размер";
pub const P720: &str = "720p";
pub const P1080: &str = "1080p";
pub const P1440: &str = "1440p";
pub const P2160: &str = "4K";
pub const SOURCE: &str = "Исходный";
pub const FPS: &str = "Кадров в секунду";
pub const QUALITY: &str = "Качество";
pub const LOW: &str = "Низкое";
pub const MEDIUM: &str = "Среднее";
pub const HIGH: &str = "Высокое";
pub const LOOP: &str = "Зациклить";
pub const NO_SOUND: &str = "GIF без звука";
pub const CANCEL: &str = "Отмена";
pub const SAVE_AS: &str = "Сохранить как…";
/// Title of the system dialog that picks the file.
pub const PICK_FILE: &str = "Куда сохранить видео";
pub const MP4_FILES: &str = "Видео MP4";
pub const GIF_FILES: &str = "Анимация GIF";
/// The ring of the header and its popover.
pub const QUEUE: &str = "Очередь экспорта";
pub const QUEUED: &str = "в очереди";
pub const CANCELLED: &str = "отменён";
pub const CANCEL_JOB: &str = "Отменить экспорт";
pub const SHOW_IN_FOLDER: &str = "Показать в папке";
pub const COPY_ERROR: &str = "Копировать ошибку";

/// «47%» of a running export.
pub fn running(percent: u64) -> String {
    format!("{percent}%")
}

/// «готово · 54,0 МБ».
pub fn done(bytes: u64) -> String {
    let megabytes = format!("{:.1}", bytes as f64 / 1_048_576.0).replace('.', ",");
    format!("готово · {megabytes} МБ")
}

/// The toast of a finished export.
pub fn finished(file: &str) -> String {
    format!("Экспорт готов: {file}")
}

/// The toast of a failed export.
pub fn failed(file: &str, error: &str) -> String {
    format!("Экспорт не удался: {file}. {error}")
}
