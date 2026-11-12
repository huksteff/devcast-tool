//! State of the editor's controls that is not the project: tools, preview size, the open
//! inspector panel. Several controls only hold their state until the task named at the field
//! makes it act.

use super::lanes::annotations::AnnotId;
use super::lanes::brush::{ERASER_WIDTH, StrokeTool};
use dc_core::time::Output;
use dc_core::{ClipId, OutputTime, Range};
use eframe::egui::Pos2;

use super::clipboard::Clipboard;
use super::selection::{Item, Selected, Selection};
use super::timeline::{Least, Row, Tracks, Viewport};

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

/// Global settings panel of the inspector: a tab of its rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    Zoom,
    /// Cursor, clicks and the callout.
    Cursor,
    /// The only tab with its panel until 7.14: the frame opens on it.
    #[default]
    Keys,
    Captions,
    Sound,
    /// «Сделать красиво» by the user's AI agent (8.11).
    Ai,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// Timeline tool.
    pub tool: Tool,
    /// Sizes the preview frame now.
    pub scale: PreviewScale,
    /// Acts from 7.4 (preview quality).
    pub quality: Quality,
    /// Automatic keyframes (7.25): an annotation edit goes into a key at the playhead, a new one
    /// when there is none; off — only while the playhead stands on a key (v1 `keyframeMode`, off
    /// by default and not kept).
    pub auto_keys: bool,
    /// Keyframe markers on the annotation regions (7.12).
    pub markers: bool,
    /// Preview volume, percent; plays from 7.5.
    pub volume: f64,
    /// Snapping on the timeline: the blade sticks with it (7.8), regions from 7.9.
    pub magnet: bool,
    /// The tab of the inspector rail: its panel shows while no object is open.
    pub panel: Panel,
    /// The picked object whose panel the inspector shows instead of the tab (7.13).
    pub object: Option<Selected>,
    /// The single picked object the inspector last saw: a new one opens its panel.
    pub followed: Option<Selected>,
    /// Playhead: playback, keys and the timeline move it.
    pub playhead: OutputTime,
    /// What the timeline shows: scale, scroll, row heights, the playhead drag (7.6, 7.7).
    pub timeline: Viewport,
    /// What the hidden timeline showed: the docked one while the timeline window is open, the
    /// window's while it is closed (each keeps its own scale, mockup 7).
    pub parked: Viewport,
    /// Picked clips (7.8; regions 7.9).
    pub selection: Selection,
    /// A span of the output picked by Shift+drag on the ruler: Shift+Delete cuts it, R sets its speed.
    pub range: Option<Range<Output>>,
    /// While Shift+drag on the ruler picks the range: where it began.
    pub ranging: Option<OutputTime>,
    /// The menu or the speed popover of the range.
    pub menu: Option<RangeMenu>,
    /// The menu of kinds a click on the ghost of an annotation row opened.
    pub kinds: Option<KindMenu>,
    /// A clip being dragged to another place of the track.
    pub drag: Option<ClipDrag>,
    /// Regions being dragged, an edge being dragged or a rubber band being drawn (7.9).
    pub gesture: Option<Gesture>,
    /// Copied regions or key for Ctrl+V (7.9, 7.25).
    pub clipboard: Clipboard,
    /// Empty rows the user added (7.10): the project has no empty tracks.
    pub least: Least,
    /// The row whose «remove?» question is open by its ×.
    pub remove: Option<Row>,
    /// The annotation track whose header is dragged to another place (layer order).
    pub lift: Option<u32>,
    /// A gesture on the preview: the zoom frame or an annotation box (7.21, 7.22).
    pub stage: Option<super::stage::Drag>,
    /// The draft of a panel control held with the pointer the preview shows (`rows::drafting`):
    /// when the drag ends the preview goes back to the project.
    pub panel_draft: Option<Vec<dc_ops::Step>>,
    /// Tool of a left drag in the picked drawing, chosen on the brush bar (7.24); Shift erases
    /// with either.
    pub brush: StrokeTool,
    /// Thickness of a new eraser stroke, px at 1080p, `WIDTH_MIN..=WIDTH_MAX`.
    pub eraser: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            scale: PreviewScale::default(),
            quality: Quality::default(),
            auto_keys: false,
            markers: true,
            volume: 80.0,
            magnet: true,
            panel: Panel::default(),
            object: None,
            followed: None,
            playhead: OutputTime::ZERO,
            timeline: Viewport::default(),
            parked: Viewport::default(),
            selection: Selection::default(),
            range: None,
            ranging: None,
            menu: None,
            kinds: None,
            drag: None,
            gesture: None,
            clipboard: Clipboard::default(),
            least: Least::default(),
            remove: None,
            lift: None,
            stage: None,
            panel_draft: None,
            brush: StrokeTool::Pencil,
            eraser: ERASER_WIDTH,
        }
    }
}

impl View {
    /// Keeps what still fits the project after it changed: picked clips and regions its rows
    /// `tracks` show, a range inside its length `end`; a gesture over the old project ends.
    pub fn keep(&mut self, tracks: &Tracks, end: OutputTime) {
        self.selection.keep(tracks);
        self.gesture = None;
        self.lift = None;
        self.remove = self
            .remove
            .filter(|r| tracks.rows.iter().any(|t| t.row == *r));
        self.range = self.range.filter(|r| r.end() <= end);
        if self.range.is_none() {
            self.menu = None;
        }
        self.drag = self.drag.filter(|d| {
            tracks
                .rows
                .iter()
                .flat_map(|t| &t.regions)
                .any(|r| r.clip == Some(d.clip))
        });
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

/// The menu of kinds of a new annotation: where it would lie and where the ghost was clicked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KindMenu {
    pub track: u32,
    pub range: Range<Output>,
    pub at: Pos2,
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

/// A gesture on the regions of the timeline (mockup 2a, 2b).
#[derive(Debug, Clone, PartialEq)]
pub enum Gesture {
    /// The picked regions follow the pointer: the time under it at the press, the row (index
    /// in `Tracks::rows`) and the region it was pressed on.
    Move {
        grab: OutputTime,
        row: usize,
        item: Item,
    },
    /// An edge (`end` or the start) of one region follows the pointer.
    Edge { target: Target, end: bool },
    /// A key marker of annotation `id` at output `from` follows the pointer, pressed at time
    /// `grab` on row `row` (7.12).
    Key {
        id: AnnotId,
        from: OutputTime,
        grab: OutputTime,
        row: usize,
    },
    /// A rubber band from `from` picks the regions it touches; `base` stays picked (Ctrl).
    Band { from: Pos2, base: Selection },
}

/// What has its edge dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Region(Item),
    /// A speed band of the clips row: where it is, its factor.
    Speed {
        range: Range<Output>,
        factor: f64,
    },
}
