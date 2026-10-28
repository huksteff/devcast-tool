//! Errors of encoding. Every message says what happened and what to do.

use std::path::PathBuf;

use crate::codec::Codec;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error(
        "ни один кодек H.264 не работает на этом компьютере ({details}): обнови драйвер видеокарты; если не поможет — переустанови программу"
    )]
    NoCodec { details: String },

    #[error(
        "кодек {codec} кодирует {width}×{height} со скоростью {measured:.0} кадров/с, а для записи {fps} кадров/с нужно не меньше {needed:.0} (с запасом): выбери частоту ниже или область меньше"
    )]
    TooSlow {
        codec: Codec,
        measured: f64,
        needed: f64,
        fps: u32,
        width: u32,
        height: u32,
    },

    #[error(
        "размер кадра {width}×{height} нельзя закодировать: ширина и высота должны быть чётными и не меньше 2"
    )]
    FrameSize { width: u32, height: u32 },

    #[error(
        "кодек {codec}: не удалось {action}: {source}. Перезапусти запись; если повторяется, обнови драйвер видеокарты"
    )]
    Codec {
        codec: Codec,
        action: &'static str,
        source: ffmpeg_next::Error,
    },

    #[error(
        "кодек звука FLAC: не удалось {action}: {source}. Перезапусти запись; если повторяется, переустанови программу"
    )]
    Audio {
        action: &'static str,
        source: ffmpeg_next::Error,
    },

    #[error(
        "кодек {codec} выдал пакет без времени: обнови драйвер видеокарты или выбери другой кодек"
    )]
    NoTimestamp { codec: Codec },

    #[error(
        "не удалось {action} файл {path}: {source}. Проверь, что диск не переполнен и папка доступна для записи"
    )]
    File {
        action: &'static str,
        path: PathBuf,
        source: ffmpeg_next::Error,
    },
}

impl EncodeError {
    pub(crate) fn codec(
        codec: Codec,
        action: &'static str,
    ) -> impl FnOnce(ffmpeg_next::Error) -> Self {
        move |source| Self::Codec {
            codec,
            action,
            source,
        }
    }

    pub(crate) fn audio(action: &'static str) -> impl FnOnce(ffmpeg_next::Error) -> Self {
        move |source| Self::Audio { action, source }
    }

    pub(crate) fn file(
        action: &'static str,
        path: &std::path::Path,
    ) -> impl FnOnce(ffmpeg_next::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::File {
            action,
            path,
            source,
        }
    }
}
