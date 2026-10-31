//! Errors of audio. Every message says what happened and what to do.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error(transparent)]
    Capture(#[from] dc_capture::CaptureError),

    #[error(transparent)]
    Media(#[from] dc_media::MediaError),

    #[error(
        "не удалось {action} звук {path}: {source}. Проверь, что файл на месте и не повреждён; если файл в порядке — сообщи о баге"
    )]
    File {
        action: &'static str,
        path: PathBuf,
        source: ffmpeg_next::Error,
    },

    #[error("в файле {path} нет дорожки {stream}: файл подменили, верни исходный")]
    NoStream { path: PathBuf, stream: usize },

    #[error(
        "«{}» — не звук в поддерживаемом формате: подходят mp3, wav, aac, m4a, flac и ogg. Перекодируй файл в один из них", .0.display()
    )]
    NotSound(PathBuf),

    #[error("нет файла «{}»: проверь путь", .0.display())]
    NotFound(PathBuf),

    #[error("в файле «{}» нет звука: выбери файл со звуковой дорожкой", .0.display())]
    NoSound(PathBuf),

    #[error("запись {0} не собрана: открой проект в DevCast, он восстановит её, затем повтори")]
    Unfinished(PathBuf),

    #[error("{what} {value} вне допустимого: задай значение в пределах {range}")]
    Level {
        what: &'static str,
        value: f32,
        range: &'static str,
    },

    #[error("не удалось изменить скорость звука до {speed}×: {source}. Сообщи о баге")]
    Tempo {
        speed: f64,
        source: ffmpeg_next::Error,
    },

    #[error("не удалось измерить громкость (EBU R128): {0}. Сообщи о баге")]
    Loudness(ebur128::Error),

    #[error(transparent)]
    Project(#[from] dc_project::ProjectError),

    #[error(
        "не удалось прочитать или записать кэш волны {path}: {source}. Проверь место на диске; папку cache/ проекта можно удалить, она построится заново"
    )]
    Cache {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("не удалось пересчитать частоту звука {path}: {message}. Сообщи о баге")]
    Resample { path: PathBuf, message: String },

    #[error(
        "нет устройства вывода звука: подключи колонки или наушники и включи воспроизведение снова"
    )]
    NoOutputDevice,

    #[error(
        "не удалось играть звук на «{device}»: {message}. Проверь устройство в настройках звука Windows и включи воспроизведение снова"
    )]
    Output { device: String, message: String },
}

impl AudioError {
    pub(crate) fn file(
        action: &'static str,
        path: &Path,
    ) -> impl FnOnce(ffmpeg_next::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::File {
            action,
            path,
            source,
        }
    }

    pub(crate) fn cache(path: &Path, source: std::io::Error) -> Self {
        Self::Cache {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn resample(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::Resample {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    }
}
