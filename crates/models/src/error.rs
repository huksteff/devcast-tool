//! Errors of the model download. Every message says what happened and what to do.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ModelsError {
    #[error("не удалось {action} «{}»: {source}. Проверь, что диск не переполнен и файл не занят другой программой", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "не удалось скачать модель с {url}: {source}. Проверь интернет и запусти ещё раз: загрузка продолжится с того же места"
    )]
    Network { url: String, source: std::io::Error },

    #[error(
        "сервер ответил кодом {status} на запрос модели {url}. Попробуй позже; если повторяется, сообщи о проблеме"
    )]
    Status { url: String, status: u16 },

    #[error(
        "сервер прислал не тот кусок модели ({url}: ждали байты с {expected}, пришло «{content_range}»). Запусти ещё раз; если повторяется, удали «{}»", part.display()
    )]
    BadRange {
        url: String,
        expected: u64,
        content_range: String,
        part: PathBuf,
    },

    #[error("скачанная модель «{}» повреждена или не та (SHA-256 {actual}, ждали {expected}). Файл удалён: запусти ещё раз, модель скачается заново", path.display())]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("загрузка модели отменена; в следующий раз она продолжится с того же места")]
    Cancelled,

    #[error(transparent)]
    Project(#[from] dc_project::ProjectError),
}

impl ModelsError {
    pub fn io(action: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io {
            action,
            path,
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, ModelsError>;
