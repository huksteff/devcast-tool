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
        "в картинке «{path}» {frames} кадр(ов), кадра {frame} нет. Это ошибка программы: сообщи разработчику"
    )]
    NoPictureFrame {
        path: String,
        frame: u32,
        frames: usize,
    },

    #[error("экспорт отменён, файл не записан")]
    Cancelled,

    #[error(
        "не удалось запустить поток очереди экспорта: {0}. Закрой лишние программы и попробуй снова"
    )]
    Thread(std::io::Error),

    #[error("звук проекта не свёлся: {0}")]
    Sound(Box<dyn std::error::Error + Send + Sync>),

    #[error("{0}")]
    Scene(Box<dyn std::error::Error + Send + Sync>),

    #[error("готовое видео пустое: всё вырезано. Верни часть (devcast undo) и экспортируй снова")]
    NothingLeft,

    #[error("частота {fps} кадров/с для {format} недоступна: выбери одну из {allowed}")]
    Fps {
        fps: u32,
        format: &'static str,
        allowed: String,
    },

    #[error(
        "120 кадров/с — только для записей на 120 кадров/с, а в проекте самая частая {source_fps:.0}: выбери 30 или 60"
    )]
    Fps120 { source_fps: f64 },

    #[error("не удалось {action} «{}»: {source}. Проверь, что папка есть и доступна для записи", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("GIF не записался: {0}. Проверь, что диск не переполнен и папка доступна для записи")]
    Gif(String),

    #[error("GIF бывает 720p, 1080p или исходного размера: выбери один из них")]
    GifResolution,

    #[error(transparent)]
    Gpu(#[from] dc_render::GpuError),

    #[error(transparent)]
    Project(#[from] dc_project::ProjectError),

    #[error(transparent)]
    Media(#[from] dc_media::MediaError),

    #[error(transparent)]
    Render(#[from] dc_render::RenderError),

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
