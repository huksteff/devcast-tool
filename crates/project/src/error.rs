//! Errors of the project folder. Every message says what happened and what to do.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("не удалось {action} «{}»: {source}. Проверь, что путь существует, диск не переполнен и файл не занят другой программой", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("«{}» не похоже на проект DevCast: нет файла project.json. Открой папку *.dcproj или создай проект заново", path.display())]
    NotAProject { path: PathBuf },

    #[error("папка «{}» уже существует: выбери другое имя или открой её как проект", path.display())]
    AlreadyExists { path: PathBuf },

    #[error("файл «{}» повреждён: {message}. Восстанови его из session.json (последняя рабочая версия) или из резервной копии", path.display())]
    Corrupt { path: PathBuf, message: String },

    #[error(
        "проект создан более новой версией DevCast (схема {found}, эта программа знает до {supported}): обнови программу"
    )]
    TooNew { found: u32, supported: u32 },

    #[error(
        "проект нарушает целостность: {0}. Это баг сохранения; отмени последнее действие или сообщи о проблеме"
    )]
    Inconsistent(String),

    #[error(
        "проект уже открыт: {holder}. Закрой его там или отправляй команды через открытый редактор (devcast сделает это сам)"
    )]
    Locked { holder: String },

    #[error(
        "не найдена папка настроек приложения: переменная окружения APPDATA не задана. Запусти devcast из обычного сеанса Windows"
    )]
    NoSettingsDir,
}

impl ProjectError {
    pub(crate) fn io<'a>(
        action: &'static str,
        path: &'a Path,
    ) -> impl FnOnce(std::io::Error) -> Self + 'a {
        move |source| Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, ProjectError>;
