//! Errors of capture. Every message says what happened and what to do.

use std::path::{Path, PathBuf};

use crate::config::ConfigError;
use crate::source::{Source, SourceError};

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Source(#[from] SourceError),

    #[error(transparent)]
    Encode(#[from] dc_encode::EncodeError),

    #[error(transparent)]
    Project(#[from] dc_project::ProjectError),

    #[error(
        "не удалось {action} {path}: {source}. Проверь, что диск не переполнен и папка доступна"
    )]
    File {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("в папке {0} уже есть запись: выбери пустую папку")]
    RecordingExists(PathBuf),

    #[error("файл {path} повреждён ({message}): эту запись не восстановить автоматически")]
    BadManifest { path: PathBuf, message: String },

    #[error(
        "в записи {0} нет ни одного целого сегмента: запись оборвалась в первые секунды, восстанавливать нечего"
    )]
    EmptyRecording(PathBuf),

    #[error(
        "монитора {index} нет, подключено {count}: выбери другой (список — devcast ls sources)"
    )]
    MonitorNotFound { index: u32, count: usize },

    #[error("окна {0} больше нет — его закрыли: обнови список источников (devcast ls sources)")]
    WindowNotFound(u64),

    #[error("окно {0} свёрнуто: разверни его, тогда будет видна картинка")]
    WindowMinimized(u64),

    #[error(
        "область {0} не лежит целиком на одном мониторе: выдели область в пределах одного экрана"
    )]
    AreaOffMonitors(Source),

    #[error(
        "область {area} не помещается в кадр монитора {width}×{height}: разрешение монитора изменилось, выдели область заново"
    )]
    AreaOutsideFrame {
        area: Source,
        width: u32,
        height: u32,
    },

    #[error(
        "эта версия Windows не умеет убирать курсор из захвата: обнови Windows 10 до версии 2004 или новее"
    )]
    CursorUnsupported,

    #[error(
        "захват не прислал ни одного кадра за {seconds} с: проверь, что источник виден на экране, и попробуй снова"
    )]
    NoFrame { seconds: u64 },

    #[error(
        "нет устройства вывода звука: подключи колонки или наушники либо выключи запись системного звука"
    )]
    NoOutputDevice,

    #[error(
        "не удалось {action}: {message}. Проверь звуковые устройства в настройках Windows и перезапусти запись"
    )]
    Audio {
        action: &'static str,
        message: String,
    },

    #[error("микрофона «{0}» нет: подключи его, выбери другой из списка или выключи микрофон")]
    MicrophoneNotFound(String),

    #[error(
        "устройство «{device}» отдаёт звук в формате {format}, а запись умеет только 32-битный float: выбери другое устройство"
    )]
    AudioFormat { device: String, format: String },

    #[error(
        "не удалось открыть звуковое устройство «{device}»: {message}. Проверь, что оно подключено и доступно в настройках конфиденциальности Windows (микрофон)"
    )]
    AudioDevice { device: String, message: String },

    #[error(
        "не удалось {action}: {message}. Перезапусти запись; если повторяется, обнови драйвер видеокарты и Windows"
    )]
    Platform {
        action: &'static str,
        message: String,
    },
}

impl CaptureError {
    pub(crate) fn file(action: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::File {
            action,
            path,
            source,
        }
    }

    pub(crate) fn audio(action: &'static str, error: impl std::fmt::Display) -> Self {
        Self::Audio {
            action,
            message: error.to_string(),
        }
    }

    pub(crate) fn audio_device(device: &str, error: cpal::Error) -> Self {
        Self::AudioDevice {
            device: device.to_owned(),
            message: error.to_string(),
        }
    }

    pub(crate) fn platform(action: &'static str) -> impl FnOnce(String) -> Self {
        move |message| Self::Platform { action, message }
    }
}
