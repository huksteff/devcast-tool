//! Errors of audio. Every message says what happened and what to do.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error(transparent)]
    Capture(#[from] dc_capture::CaptureError),

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

    #[error("не удалось пересчитать частоту звука {path}: {message}. Сообщи о баге")]
    Resample { path: PathBuf, message: String },
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

    pub(crate) fn resample(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::Resample {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    }
}
