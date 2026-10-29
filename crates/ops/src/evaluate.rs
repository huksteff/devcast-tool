//! `evaluate(project, t) -> Scene` of the whole project (ARCHITECTURE, decision 4; decision 015):
//! every effect adds its part to the scene in a fixed order. The one source of scenes for the
//! preview, the export and `devcast frame`/`sheet`.

use dc_core::{
    CursorTracks, KeyTracks, OutputTime, PixelSize, Project, StateError, TimeDelta, TimeMap,
};
use dc_fx_callout::{Anchor, Callout, CalloutState};
use dc_fx_crop::Crop;
use dc_fx_cursor::Cursor;
use dc_fx_keys::Keys;
use dc_fx_zoom::{CameraTrack, ZoomEffect};
use dc_render::{RenderError, Scene, TextMeasure};

/// The project's scene can not be built.
#[derive(Debug, thiserror::Error)]
pub enum EvaluateError {
    #[error(transparent)]
    State(#[from] StateError),

    #[error(transparent)]
    Text(#[from] RenderError),

    #[error("в проекте нет клипов: добавь видео (devcast clip add <видео>) и попробуй снова")]
    Empty,
}

/// A project ready to be evaluated at any output moment: the time map and the effect states are
/// read once, not per frame.
#[derive(Debug)]
pub struct Evaluator<'p> {
    project: &'p Project,
    map: TimeMap,
    canvas: PixelSize,
    camera: CameraTrack,
    cursor: dc_fx_cursor::Prepared,
    keys: dc_fx_keys::Prepared,
    callout: CalloutState,
    captions: dc_fx_captions::Prepared,
}

impl<'p> Evaluator<'p> {
    /// `cursors` and `keys` — the telemetry of the recordings on the track (decision 015); the
    /// caller reads it from the project folder. `measure` holds the fonts the frames are drawn
    /// with: captions are broken into lines here (decision 016).
    pub fn new(
        project: &'p Project,
        cursors: CursorTracks,
        keys: KeyTracks,
        measure: &mut TextMeasure,
    ) -> Result<Self, EvaluateError> {
        let crop = project.state::<Crop>()?;
        let canvas = dc_fx_crop::canvas(project, &crop.settings).ok_or(EvaluateError::Empty)?;
        let map = TimeMap::build(project);
        let camera = CameraTrack::new(
            project,
            &project.state::<ZoomEffect>()?,
            &map,
            canvas,
            &cursors,
        );
        let cursor = project.state::<Cursor>()?;
        let keys = dc_fx_keys::Prepared::new(project.state::<Keys>()?, &keys, &map);
        Ok(Self {
            project,
            map,
            canvas,
            camera,
            cursor: dc_fx_cursor::Prepared::new(project, cursor, cursors),
            keys,
            callout: project.state::<Callout>()?,
            captions: dc_fx_captions::Prepared::new(
                project.state::<dc_fx_captions::CaptionsEffect>()?,
                canvas,
                measure,
            )?,
        })
    }

    /// Size of every scene: the output frame before the export scales it.
    pub fn canvas(&self) -> PixelSize {
        self.canvas
    }

    pub fn duration(&self) -> TimeDelta {
        self.map.duration()
    }

    pub fn map(&self) -> &TimeMap {
        &self.map
    }

    /// What output moment `t` shows; black outside `0..duration`.
    pub fn evaluate(&self, t: OutputTime) -> Scene {
        let mut scene = Scene::new(self.canvas);
        dc_fx_cut::evaluate(self.project, &self.map, t, &mut scene);
        self.camera.evaluate(t, &mut scene);
        dc_fx_cursor::evaluate(&self.cursor, self.project, &self.map, t, &mut scene);
        let anchor = dc_fx_cursor::pointer(&self.cursor, self.project, &self.map, t, scene.size)
            .map(|p| Anchor {
                tip: p.tip,
                arrow: p.arrow,
                k: p.k,
            });
        dc_fx_callout::evaluate(&self.callout, &self.map, t, anchor, &mut scene);
        dc_fx_captions::evaluate(&self.captions, &self.map, t, &mut scene);
        dc_fx_keys::evaluate(&self.keys, t, &mut scene);
        scene
    }
}
