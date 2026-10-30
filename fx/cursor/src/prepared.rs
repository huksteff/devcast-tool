//! The cursor of a project prepared once by `dc_ops::Evaluator::new`, and where the clip of an
//! output moment is shown.

use std::collections::BTreeMap;

use dc_core::{
    CursorPoint, CursorShape, CursorTracks, OutputTime, PixelSize, Point, Project, Rect,
    SourceTime, TimeDelta, TimeMap,
};
use dc_render::{PxRect, Video, VideoSource};

use crate::click_state::{ClickChange, ClickStyle};
use crate::path::{Path, Sway};
use crate::state::{CursorSettings, CursorState};

/// Arrow height in px at size 1 in a clip 1920 px wide (v1 `h = 28 · size · W / 1920`).
const ARROW_PX: f64 = 28.0;
/// v1 sizes and speeds (arrow, click effects, callout, sway) are ref-px: px of a clip this wide.
pub(crate) const REFERENCE_WIDTH: f64 = 1920.0;
/// A pointer this far outside the crop still counts as inside it (v1, fractions of the crop).
const EDGE: f64 = 1e-6;

/// The cursor of one recording, ready for every frame.
#[derive(Debug)]
pub(crate) struct Recording {
    pub path: Path,
    pub shapes: Vec<CursorShape>,
    pub clicks: Vec<CursorPoint>,
}

/// The cursor of a project: its settings and the paths of its recordings.
#[derive(Debug)]
pub struct Prepared {
    pub(crate) settings: CursorSettings,
    pub(crate) recordings: BTreeMap<String, Recording>,
    changes: Vec<ClickChange>,
    /// The longest click effect, of all clicks and of the changed ones.
    pub(crate) longest_click: TimeDelta,
}

impl Prepared {
    /// Paths of every recording on the track that has mouse events; imported videos have none
    /// and show no cursor.
    pub fn new(project: &Project, state: CursorState, tracks: CursorTracks) -> Self {
        let CursorState { settings, clicks } = state;
        let spring = settings.smoothing.spring();
        let sway = (settings.sway > 0.0).then(|| Sway {
            spring: settings.smoothing.sway_spring(),
            strength: settings.sway,
        });
        let mut recordings = BTreeMap::new();
        for (media, track) in tracks {
            let Some(clip) = project.clips.iter().find(|c| c.media.path == media) else {
                continue;
            };
            if track.moves.is_empty() {
                continue;
            }
            let size = clip.media.size;
            let aspect = f64::from(size.height) / f64::from(size.width);
            let path = Path::new(
                track.moves,
                spring.as_ref(),
                sway.as_ref(),
                clip.media.duration,
                aspect,
            );
            let recording = Recording {
                path,
                shapes: track.shapes,
                clicks: track.clicks,
            };
            recordings.insert(media, recording);
        }
        let longest_click = clicks
            .iter()
            .filter_map(|c| c.duration)
            .fold(settings.click.duration, TimeDelta::max);
        Self {
            settings,
            recordings,
            changes: clicks,
            longest_click,
        }
    }

    /// The effect of the press of `media` at `at`: the common style with its change, if any.
    pub(crate) fn click_style(&self, media: &str, at: SourceTime) -> ClickStyle {
        let style = &self.settings.click;
        self.changes
            .iter()
            .find(|c| c.is_for(media, at))
            .map_or_else(|| style.clone(), |c| c.applied_to(style))
    }

    /// Where the recording playing at output moment `t` is shown in a `frame`-sized scene;
    /// `None` outside the video or over a clip without telemetry.
    pub(crate) fn place<'a>(
        &'a self,
        project: &Project,
        map: &'a TimeMap,
        t: OutputTime,
        frame: PixelSize,
    ) -> Option<Place<'a>> {
        let segment = map.segment_at(t)?;
        let recording = self.recordings.get(&segment.media)?;
        let clip = project.clip(segment.clip)?;
        let source = segment.source_at(t);
        let video = VideoSource {
            media: segment.media.clone(),
            time: source,
        };
        let content = Video::fitted(video, clip.media.size, clip.crop, frame).rect;
        Some(Place {
            recording,
            media: &segment.media,
            source,
            start: segment.output_start,
            content,
            crop: clip.crop,
            arrow: ARROW_PX * self.settings.size * f64::from(content.w) / REFERENCE_WIDTH,
        })
    }
}

/// A recording at an output moment, as shown in the frame.
#[derive(Debug)]
pub(crate) struct Place<'a> {
    pub recording: &'a Recording,
    pub media: &'a str,
    pub source: SourceTime,
    /// Output start of the segment playing: motion blur does not reach across a cut.
    pub start: OutputTime,
    /// The clip's crop in the frame, px.
    pub content: PxRect,
    pub crop: Rect,
    /// Height of the arrow (v1 `h`), px of the frame: the cursor and click effects scale by it.
    pub arrow: f64,
}

impl Place<'_> {
    /// Frame px of point `p` of the recorded picture; `None` outside the crop.
    pub fn on_frame(&self, p: Point) -> Option<[f64; 2]> {
        let local = self.crop.to_local(p);
        let inside = |v: f64| (-EDGE..=1.0 + EDGE).contains(&v);
        let c = self.content;
        (inside(local.x) && inside(local.y)).then(|| {
            [
                f64::from(c.x) + local.x * f64::from(c.w),
                f64::from(c.y) + local.y * f64::from(c.h),
            ]
        })
    }
}
