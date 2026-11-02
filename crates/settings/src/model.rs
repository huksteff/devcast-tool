//! What the app remembers in `settings.json` and its defaults. A field missing in the file takes
//! its default, so the file of an older version still loads.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dc_capture::{RecordingConfig, Source};
use serde::{Deserialize, Serialize};

use crate::keymap::EditorKeys;
use crate::placement::HudPlacement;

/// File of the settings in the app settings folder (`dc_project::settings::dir`).
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechModel {
    /// «Быстрая»: Whisper small, the default everywhere (owner's decision).
    #[default]
    Fast,
    /// «Точная»: Whisper large-v3-turbo.
    Accurate,
}

impl SpeechModel {
    pub const ALL: [Self; 2] = [Self::Fast, Self::Accurate];

    /// The Whisper model file behind the choice.
    pub const fn model(self) -> dc_models::Model {
        match self {
            Self::Fast => dc_models::Model::Small,
            Self::Accurate => dc_models::Model::LargeV3Turbo,
        }
    }
}

/// The language Whisper listens for (the CLI's `--language`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    /// Detected by Whisper.
    #[default]
    Auto,
    Russian,
    English,
}

impl Language {
    pub fn code(self) -> Option<String> {
        match self {
            Self::Auto => None,
            Self::Russian => Some("ru".to_owned()),
            Self::English => Some("en".to_owned()),
        }
    }
}

/// The tracks of the recordings to listen to (the CLI's `--tracks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Listen {
    /// The microphone (a recording without it — its first track), as `devcast captions gen`.
    #[default]
    Mic,
    System,
    /// Both; the microphone wins where they overlap.
    Both,
}

impl Listen {
    pub fn tracks(self) -> Vec<String> {
        match self {
            Self::Mic => Vec::new(),
            Self::System => vec!["system".to_owned()],
            Self::Both => vec!["mic".to_owned(), "system".to_owned()],
        }
    }
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

/// Sizes of the editor panels, points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorLayout {
    /// Height of the timeline under the preview (mockup: about 300).
    pub timeline_height: f32,
    /// Height factor of the timeline rows, 0.5..3 (Shift+wheel).
    pub rows: f32,
    /// The timeline is in its own window (7.26): the editor opens with it.
    pub timeline_window: bool,
    /// Where the timeline window was when it closed last; `None` — centred on the HUD monitor.
    pub timeline_place: Option<WindowPlace>,
}

impl Default for EditorLayout {
    fn default() -> Self {
        Self {
            timeline_height: 300.0,
            rows: 1.0,
            timeline_window: false,
            timeline_place: None,
        }
    }
}

/// A window on the desktop, points of the main window: its outer corner and its inner size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPlace {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Groups of the inspector panels the user opened or closed: `panel.group` (`keys.fine`) → open.
/// A group not here has the default of its panel (v2 remembers, v1 did not).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InspectorGroups(pub BTreeMap<String, bool>);

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
    /// Language and tracks of «Создать субтитры»: the settings window and the captions panel.
    pub speech_language: Language,
    pub speech_tracks: Listen,
    pub updates: Updates,
    /// Where the HUD stood on each monitor.
    pub hud: HudPlacement,
    /// The last selected area (always `area:…`), offered when the selection opens again.
    pub last_area: Option<Source>,
    /// Sizes of the editor panels the user dragged.
    pub editor: EditorLayout,
    /// Editor shortcuts the user reassigned (`keymap.rs`).
    pub editor_keys: EditorKeys,
    /// Open and closed groups of the editor's inspector.
    pub inspector: InspectorGroups,
    /// Folders of the projects opened in the editor, newest first, at most 16 (the «Проекты» menu).
    pub recent_projects: Vec<PathBuf>,
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
            speech_language: Language::default(),
            speech_tracks: Listen::default(),
            updates: Updates::default(),
            hud: HudPlacement::default(),
            last_area: None,
            editor: EditorLayout::default(),
            editor_keys: EditorKeys::default(),
            inspector: InspectorGroups::default(),
            recent_projects: Vec::new(),
        }
    }
}

fn default_recordings_dir() -> PathBuf {
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Videos")))
        .unwrap_or_default()
        .join("DevCast")
}
