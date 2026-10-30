//! The app settings, one file `%APPDATA%\DevCast\settings.json`: the HUD and the settings
//! window read and change the same [`Store`], so a change in one is seen in the other at once
//! (owner's decision on the mockups). Each change is written atomically right away.

use std::path::{Path, PathBuf};

use dc_capture::{RecordingConfig, Source};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::hud::placement::HudPlacement;

/// File of the settings in the app settings folder.
pub const FILE: &str = "settings.json";

/// Countdown before the recording starts, seconds: 0, 3, 5 or 10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum Countdown {
    Off,
    #[default]
    Three,
    Five,
    Ten,
}

impl Countdown {
    pub const ALL: [Self; 4] = [Self::Off, Self::Three, Self::Five, Self::Ten];

    pub const fn seconds(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Three => 3,
            Self::Five => 5,
            Self::Ten => 10,
        }
    }
}

impl TryFrom<u32> for Countdown {
    type Error = String;

    fn try_from(seconds: u32) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|c| c.seconds() == seconds)
            .ok_or_else(|| format!("отсчёт {seconds} с не поддерживается: выбери 0, 3, 5 или 10"))
    }
}

impl From<Countdown> for u32 {
    fn from(countdown: Countdown) -> Self {
        countdown.seconds()
    }
}

/// Speech recognition model (the CLI's `--model fast|accurate`, decision 010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechModel {
    /// «Быстрая»: Whisper small, the default everywhere (owner's decision).
    #[default]
    Fast,
    /// «Точная»: Whisper large-v3-turbo.
    Accurate,
}

/// Global shortcuts as text (`Ctrl+Shift+R`); `None` — not assigned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub start_stop: Option<String>,
    pub pause: Option<String>,
    pub cancel: Option<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            start_stop: Some("Ctrl+Shift+R".to_owned()),
            pause: Some("Ctrl+Shift+P".to_owned()),
            cancel: Some("Ctrl+Shift+X".to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Updates {
    /// Check every 6 hours.
    pub auto_check: bool,
    pub beta: bool,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            auto_check: true,
            beta: false,
        }
    }
}

/// Everything the app remembers. A field missing in the file takes its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Where recordings and their projects are created.
    pub recordings_dir: PathBuf,
    /// Source, fps, system sound and microphone of the next recording.
    pub recording: RecordingConfig,
    pub countdown: Countdown,
    /// Exclude the HUD from capture.
    pub hide_hud: bool,
    pub hotkeys: Hotkeys,
    pub speech_model: SpeechModel,
    pub updates: Updates,
    /// Where the HUD stood on each monitor.
    pub hud: HudPlacement,
}

impl Default for AppSettings {
    /// The primary monitor (`screen:1` until the source list says otherwise), recordings in
    /// «Видео\DevCast» of the user.
    fn default() -> Self {
        Self {
            recordings_dir: default_recordings_dir(),
            recording: RecordingConfig::new(Source::Monitor(1)),
            countdown: Countdown::default(),
            hide_hud: true,
            hotkeys: Hotkeys::default(),
            speech_model: SpeechModel::default(),
            updates: Updates::default(),
            hud: HudPlacement::default(),
        }
    }
}

fn default_recordings_dir() -> PathBuf {
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Videos")))
        .unwrap_or_default()
        .join("DevCast")
}

/// The settings in memory and their file.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    settings: AppSettings,
}

impl Store {
    /// Reads `path`; no file yet — the defaults.
    pub fn load(path: &Path) -> Result<Self, AppError> {
        let settings = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| AppError::BadSettings {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
            Err(e) => return Err(AppError::settings_file(path, e)),
        };
        Ok(Self {
            path: path.to_path_buf(),
            settings,
        })
    }

    pub fn get(&self) -> &AppSettings {
        &self.settings
    }

    /// Changes the settings with `change` and writes the file if anything changed.
    pub fn update(&mut self, change: impl FnOnce(&mut AppSettings)) -> Result<(), AppError> {
        let mut next = self.settings.clone();
        change(&mut next);
        if next == self.settings {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::settings_file(parent, e))?;
        }
        dc_project::atomic::write_json(&self.path, &next)?;
        self.settings = next;
        Ok(())
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
