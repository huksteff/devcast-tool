//! Errors of capture. Every message says what happened and what to do.

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
        "не удалось {action}: {message}. Перезапусти запись; если повторяется, обнови драйвер видеокарты и Windows"
    )]
    Platform {
        action: &'static str,
        message: String,
    },
}

impl CaptureError {
    pub(crate) fn platform(action: &'static str) -> impl FnOnce(String) -> Self {
        move |message| Self::Platform { action, message }
    }
}
