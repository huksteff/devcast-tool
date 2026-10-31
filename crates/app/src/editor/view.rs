//! State of the editor's controls that is not the project: tools, preview size, the open
//! inspector panel. Several controls only hold their state until the task named at the field
//! makes it act.

use dc_core::time::Output;
use dc_core::{ClipId, OutputTime, Project, Range};

use super::selection::Selection;
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
    /// Timeline tool.
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
    /// Snapping on the timeline: the blade sticks with it (7.8), regions from 7.9.
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
    /// Picked clips (7.8; regions 7.9).
    pub selection: Selection,
    /// A span of the output picked by Shift+drag on the ruler: Shift+Delete cuts it, R sets its speed.
    pub range: Option<Range<Output>>,
    /// While Shift+drag on the ruler picks the range: where it began.
    pub ranging: Option<OutputTime>,
    /// The menu or the speed popover of the range.
    pub menu: Option<RangeMenu>,
    /// A clip being dragged to another place of the track.
    pub drag: Option<ClipDrag>,
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
            selection: Selection::default(),
            range: None,
            ranging: None,
            menu: None,
            drag: None,
        }
    }
}

impl View {
    /// Keeps what still fits `project` after it changed: picked clips it has, a range inside
    /// its length `end`; a gesture over the old project ends.
    pub fn keep(&mut self, project: &Project, end: OutputTime) {
        self.selection.keep(project);
        self.range = self.range.filter(|r| r.end() <= end);
        if self.range.is_none() {
            self.menu = None;
        }
        self.drag = self
            .drag
            .filter(|d| project.clips.iter().any(|c| c.id == d.clip));
    }
}

/// What opens over the picked range (mockup 2d).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RangeMenu {
    /// Its length, «Вырезать участок», «Скорость участка…».
    Actions,
    /// Preset speeds and a speed of one's own, 0.25–16.
    Speed { own: f64 },
}

/// A clip dragged along the main track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipDrag {
    pub clip: ClipId,
    /// Pointer x minus the left edge of the clip when grabbed, points.
    pub grab: f32,
    /// Place of the clip on the track when grabbed, from 0.
    pub from: usize,
}
