//! `DevCast-Setup.exe`: where to install DevCast (system dialogs of Windows).

pub const TITLE: &str = "Установка DevCast";
pub const INSTALL: &str = "Установить";
pub const OTHER_FOLDER: &str = "Другая папка…";
pub const CANCEL: &str = "Отмена";
pub const PICK_FOLDER: &str = "Куда установить DevCast";
pub const NOT_PACKED: &str =
    "В этой сборке нет установщика: соберите её командой cargo xtask package.";

/// The question with the folder DevCast goes to.
pub fn question(folder: &std::path::Path) -> String {
    format!(
        "DevCast установится в папку:\n{}\n\nПрава администратора не нужны, обновления будут ставиться туда же.",
        folder.display()
    )
}

/// The installer could not run or ended with an error.
pub fn failed(error: &dyn std::fmt::Display) -> String {
    format!(
        "Установка не удалась ({error}). Попробуйте другую папку или запустите установщик ещё раз."
    )
}
