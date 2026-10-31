//! State of the editor's controls that is not the project: tools, preview size, the open
//! inspector panel. Several controls only hold their state until the task named at the field
//! makes it act.

use dc_core::OutputTime;

use super::timeline::Viewport;

/// Tool of a click on the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// «Выбор» (A): select and drag.
    #[default]
    Select,
    /// «Лезвие» (B): a click cuts the clip there.
    Blade,
}

/// Size of the preview frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewScale {
    /// Fitted into the preview area.
    #[default]
    Fit,
    /// One frame pixel per two screen pixels.
    Half,
    /// One frame pixel per screen pixel.
    Actual,
}

/// Quality of the preview frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    #[default]
    Full,
    /// Lower resolution when the PC does not keep up.
    Fast,
}

/// Panel of the inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Zoom,
    /// Cursor, clicks and the callout.
    Cursor,
    Keys,
    Captions,
    Sound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// Timeline tool; the blade cuts from 7.8.
    pub tool: Tool,
    /// Sizes the preview frame now.
    pub scale: PreviewScale,
    /// Acts from 7.4 (preview quality).
    pub quality: Quality,
    /// Automatic keyframes; acts from 7.25.
    pub auto_keys: bool,
    /// Keyframe markers on the timeline; acts from 7.12.
    pub markers: bool,
    /// Preview volume, percent; plays from 7.5.
    pub volume: f64,
    /// Snapping on the timeline; acts from 7.9.
    pub magnet: bool,
    /// Shown inspector panel.
    pub panel: Panel,
    /// Open groups of the inspector panel («Основное», «Тонкая настройка»); remembered from 7.13.
    pub basic_open: bool,
    pub fine_open: bool,
    /// Playhead: playback, keys and the timeline move it.
    pub playhead: OutputTime,
    /// What the timeline shows: scale, scroll, row heights, the playhead drag (7.6, 7.7).
    pub timeline: Viewport,
}

impl Default for View {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            scale: PreviewScale::default(),
            quality: Quality::default(),
            auto_keys: true,
            markers: true,
            volume: 80.0,
            magnet: true,
            panel: Panel::default(),
            basic_open: true,
            fine_open: false,
            playhead: OutputTime::ZERO,
            timeline: Viewport::default(),
        }
    }
}
