//! Errors of reading media files. Every message says what happened and what to do.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("не удалось открыть видео «{}»: {source}. Проверь путь; поддерживаются mp4, mov, mkv и webm", path.display())]
    Open {
        path: PathBuf,
        source: ffmpeg_next::Error,
    },

    #[error("файл «{}» не найден: проверь путь", path.display())]
    NotFound { path: PathBuf },

    #[error("«{}» — не видео в поддерживаемом формате: импортируются mp4, mov, mkv и webm. Перекодируй файл в mp4", path.display())]
    NotAVideo { path: PathBuf },

    #[error("в файле «{}» нет видео: нужен файл с видеодорожкой (mp4, mov, mkv, webm)", path.display())]
    NoVideo { path: PathBuf },

    #[error("в видео «{}» нет ни одного кадра: файл пустой или оборван, запиши или экспортируй его заново", path.display())]
    NoFrames { path: PathBuf },

    #[error("в видео «{}» нет ключевых кадров, по нему нельзя перематывать: перекодируй его (например, в mp4 H.264)", path.display())]
    NoKeyframes { path: PathBuf },

    #[error("не удалось прочитать видео «{}»: {source}. Файл повреждён или занят другой программой", path.display())]
    Read {
        path: PathBuf,
        source: ffmpeg_next::Error,
    },

    #[error(
        "декодированные кадры заняли весь лимит памяти ({limit_mb} МБ): отпускай показанные кадры или увеличь лимит пула"
    )]
    PoolFull { limit_mb: usize },

    #[error("не удалось декодировать видео «{}»: {source}. Файл повреждён; перекодируй его", path.display())]
    Decode {
        path: PathBuf,
        source: ffmpeg_next::Error,
    },

    #[error("поток декодирования видео «{}» остановился: открой видео заново", path.display())]
    Stopped { path: PathBuf },

    #[error("не удалось {action} «{}»: {source}. Проверь, что путь существует и диск не переполнен", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(transparent)]
    Project(#[from] dc_project::ProjectError),
}

impl MediaError {
    pub(crate) fn io(
        action: &'static str,
        path: &std::path::Path,
    ) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io {
            action,
            path,
            source,
        }
    }
}
