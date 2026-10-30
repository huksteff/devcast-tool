//! Errors that stop the app; each says what to do.

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Settings(#[from] dc_project::ProjectError),
    #[error("не удалось открыть журнал в {path}: {message}. Проверь права на папку настроек")]
    Log {
        path: std::path::PathBuf,
        message: String,
    },
    #[error("не удалось проверить, запущен ли DevCast: {0}. Перезапусти программу")]
    Instance(#[from] dc_ipc::IpcError),
    #[error("не запущена запись дампов падений: {0}")]
    Crash(String),
    #[error(transparent)]
    Source(#[from] dc_capture::SourceError),
    #[error(transparent)]
    Capture(#[from] dc_capture::CaptureError),
    #[error("не удалось запустить поток «{name}»: {source}. Перезапусти программу")]
    Thread {
        name: &'static str,
        source: std::io::Error,
    },
    #[error("не удалось включить глобальные горячие клавиши: {0}. Перезапусти программу")]
    Hotkeys(String),
    #[error("не удалось создать значок в трее: {0}. Перезапусти программу")]
    Tray(String),
    #[error("не удалось открыть окно: {0}. Обнови драйвер видеокарты")]
    Window(String),
    #[error("файл настроек {path} испорчен ({message}): удали его, DevCast создаст новый")]
    BadSettings {
        path: std::path::PathBuf,
        message: String,
    },
    #[error("не удалось записать или прочитать {path}: {source}. Проверь права на папку")]
    SettingsFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

impl AppError {
    pub fn settings_file(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::SettingsFile {
            path: path.to_path_buf(),
            source,
        }
    }
}
