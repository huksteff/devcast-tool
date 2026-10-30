//! Errors of setting up the interface; each says what to do.

#[derive(Debug, thiserror::Error)]
pub enum UiKitError {
    #[error(transparent)]
    FontsDir(#[from] dc_render::RenderError),
    #[error("не прочитан шрифт интерфейса {path}: {source}. Переустанови программу")]
    Font {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}
