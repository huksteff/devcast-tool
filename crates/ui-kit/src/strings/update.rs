//! Updates (8.2): the HUD's «Скачать?» popover and the update rows of the settings window.

/// Title of the popover window.
pub const TITLE: &str = "Обновление";
pub const DOWNLOAD: &str = "Скачать";
pub const NOT_NOW: &str = "Не сейчас";
pub const SKIP: &str = "Пропустить версию";
pub const RESTART: &str = "Перезапустить";
pub const CHECKING: &str = "Проверяю…";
pub const DOWNLOADING_BUTTON: &str = "Скачивается…";
pub const NOT_INSTALLED: &str = "обновления работают в установленной программе";

/// «Доступна версия 2.1.0. Скачать?».
pub fn available(version: &str) -> String {
    format!("Доступна версия {version}. Скачать?")
}

/// «Скачиваю 2.1.0… 62%».
pub fn downloading(version: &str, done: f32) -> String {
    format!("Скачиваю {version}… {}%", (done * 100.0).round())
}

/// «2.1.0 готова. Установится при выходе».
pub fn ready(version: &str) -> String {
    format!("{version} готова. Установится при выходе")
}

/// The status under the version in the settings: «последняя версия», «доступна 2.1.0».
pub const UP_TO_DATE: &str = "последняя версия";

pub fn found(version: &str) -> String {
    format!("доступна {version}")
}
