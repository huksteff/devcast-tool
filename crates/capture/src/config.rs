//! Settings of one recording: source, frame rate, audio tracks. The last ones used are kept
//! in a file of the app's settings (SCOPE: the last chosen values are remembered).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::source::Source;

/// Recording frame rate. Stored as a number: 30, 60 or 120.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum Fps {
    Fps30,
    #[default]
    Fps60,
    Fps120,
}

impl Fps {
    pub const fn get(self) -> u32 {
        match self {
            Self::Fps30 => 30,
            Self::Fps60 => 60,
            Self::Fps120 => 120,
        }
    }
}

impl TryFrom<u32> for Fps {
    type Error = ConfigError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            30 => Ok(Self::Fps30),
            60 => Ok(Self::Fps60),
            120 => Ok(Self::Fps120),
            _ => Err(ConfigError::Fps(value)),
        }
    }
}

impl From<Fps> for u32 {
    fn from(fps: Fps) -> Self {
        fps.get()
    }
}

/// What and how to record. Each audio track is separate and can be switched off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingConfig {
    pub source: Source,
    pub fps: Fps,
    /// Record system sound (what the speakers play) as its own track.
    pub system_audio: bool,
    /// Microphone device name as the system shows it; `None` — no microphone track.
    pub microphone: Option<String>,
}

impl RecordingConfig {
    /// 60 fps with system sound, no microphone.
    pub fn new(source: Source) -> Self {
        Self {
            source,
            fps: Fps::default(),
            system_audio: true,
            microphone: None,
        }
    }

    /// The settings saved last with [`Self::save`]; `None` when there are none yet.
    pub fn load(path: &Path) -> Result<Option<Self>, CaptureError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(CaptureError::file("прочитать", path)(e)),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| CaptureError::BadSettings {
                path: path.to_path_buf(),
                message: e.to_string(),
            })
    }

    /// Remembers these settings (atomically: a crash leaves the old file or the new one).
    pub fn save(&self, path: &Path) -> Result<(), CaptureError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(CaptureError::file("создать папку", parent))?;
        }
        Ok(dc_project::atomic::write_json(path, self)?)
    }

    /// Checks what the types cannot: a chosen microphone must have a name.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match &self.microphone {
            Some(name) if name.trim().is_empty() => Err(ConfigError::EmptyMicrophone),
            _ => Ok(()),
        }
    }
}

/// Recording settings that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("частота {0} кадров/с не поддерживается: выбери 30, 60 или 120")]
    Fps(u32),
    #[error(
        "у микрофона пустое имя: выбери устройство из списка или выключи микрофон (microphone: null)"
    )]
    EmptyMicrophone,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Area;

    #[test]
    fn json_round_trips() {
        let config = RecordingConfig {
            source: Source::Area(Area::new(-1700, 150, 1280, 720).unwrap()),
            fps: Fps::Fps120,
            system_audio: false,
            microphone: Some("Микрофон (Realtek Audio)".to_owned()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            json,
            r#"{"source":"area:-1700,150,1280,720","fps":120,"system_audio":false,"microphone":"Микрофон (Realtek Audio)"}"#
        );
        assert_eq!(
            serde_json::from_str::<RecordingConfig>(&json).unwrap(),
            config
        );
    }

    #[test]
    fn defaults_are_60_fps_with_system_sound() {
        let config = RecordingConfig::new(Source::Monitor(1));
        assert_eq!(config.fps.get(), 60);
        assert!(config.system_audio);
        assert_eq!(config.microphone, None);
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn only_supported_fps() {
        for fps in [30, 60, 120] {
            assert_eq!(Fps::try_from(fps).unwrap().get(), fps);
        }
        for fps in [0, 24, 59, 144] {
            assert_eq!(Fps::try_from(fps), Err(ConfigError::Fps(fps)));
        }
        let json = r#"{"source":"screen:1","fps":25,"system_audio":true,"microphone":null}"#;
        let error = serde_json::from_str::<RecordingConfig>(json).unwrap_err();
        assert!(error.to_string().contains("30, 60 или 120"), "{error}");
    }

    #[test]
    fn last_settings_are_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Настройки DevCast").join("recording.json");
        assert_eq!(RecordingConfig::load(&path).unwrap(), None);
        let config = RecordingConfig {
            microphone: Some("Микрофон (6- Usb Audio Device)".to_owned()),
            ..RecordingConfig::new(Source::Monitor(2))
        };
        config.save(&path).unwrap();
        assert_eq!(RecordingConfig::load(&path).unwrap(), Some(config));
        std::fs::write(&path, "{\"source\":").unwrap();
        let error = RecordingConfig::load(&path).unwrap_err();
        assert!(error.to_string().contains("удали его"), "{error}");
    }

    #[test]
    fn rejects_bad_fields() {
        let json = r#"{"source":"screen:0","fps":60,"system_audio":true,"microphone":null}"#;
        let error = serde_json::from_str::<RecordingConfig>(json).unwrap_err();
        assert!(error.to_string().contains("нумеруются с 1"), "{error}");
        let json =
            r#"{"source":"screen:1","fps":60,"system_audio":true,"microphone":null,"cursor":true}"#;
        assert!(serde_json::from_str::<RecordingConfig>(json).is_err());
        let mut config = RecordingConfig::new(Source::Window(42));
        config.microphone = Some("  ".to_owned());
        assert_eq!(config.validate(), Err(ConfigError::EmptyMicrophone));
    }
}
