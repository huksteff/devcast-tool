//! Projects of the editor (7.28, mockup 6): the «Проекты» menu, the question on closing with
//! unsaved changes, recovery of a working state, «Сохранить как…» and renaming in the header.

pub const RECENT: &str = "Последние проекты";
/// The mark of the project open in the editor in the recent list.
pub const CURRENT: &str = "открыт";
pub const NO_RECENT: &str = "Здесь появятся открытые проекты.";
pub const NEW: &str = "Новый проект";
pub const OPEN: &str = "Открыть…";
pub const SAVE: &str = "Сохранить";
pub const SAVE_AS: &str = "Сохранить как…";
/// Title of the folder picker of «Открыть…».
pub const PICK_PROJECT: &str = "Папка проекта .devcast";
pub const DONT_SAVE: &str = "Не сохранять";
pub const CANCEL: &str = "Отмена";
pub const UNSAVED_TEXT: &str = "Иначе изменения после последнего сохранения пропадут.";
pub const RECOVERY_TITLE: &str = "Найдена несохранённая работа";
pub const DISCARD: &str = "Отбросить";
pub const RESTORE: &str = "Восстановить";
pub const SAVE_AS_TITLE: &str = "Сохранить проект как";
pub const SAVE_AS_TEXT: &str =
    "Копия с записями появится рядом и откроется. В этом проекте останется последнее сохранённое.";
/// The name field of «Сохранить как…» and of renaming in the header.
pub const NAME: &str = "Название проекта";
pub const COPYING: &str = "Копирую проект…";
pub const RENAME: &str = "Переименовать";
/// Renaming waits until the export queue is empty: the export reads the project's files.
pub const RENAME_EXPORTING: &str =
    "Проект нельзя переименовать, пока идёт экспорт: дождитесь его конца или отмените его.";
pub const NOT_A_PROJECT: &str = "Это не папка проекта: выберите папку с расширением .devcast.";
/// A request about the open project came when the editor had none.
pub const NOTHING_OPEN: &str = "В редакторе нет открытого проекта: откройте его и повторите.";

/// «Сохранить изменения в «Урок»?».
pub fn save_title(name: &str) -> String {
    format!("Сохранить изменения в «{name}»?")
}

/// The recovery question of a project with a working state left on disk.
pub fn recovery_text(name: &str) -> String {
    format!(
        "В «{name}» остались правки, не сохранённые в проект: DevCast закрылся, не сохранив их."
    )
}

/// The name of a new project: «Новый проект», then «Новый проект 2» …
pub fn new_name(number: usize) -> String {
    match number {
        0 | 1 => NEW.to_owned(),
        n => format!("{NEW} {n}"),
    }
}

/// The name a copy is offered under: «Урок — копия».
pub fn copy_name(name: &str) -> String {
    format!("{name} — копия")
}
