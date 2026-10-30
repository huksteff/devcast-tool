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
    #[error("не удалось открыть окно: {0}. Обнови драйвер видеокарты")]
    Window(String),
}
