//! `evaluate(project, t) -> Scene` of the whole project (ARCHITECTURE, decision 4; decision 015):
//! every effect adds its part to the scene in a fixed order. The one source of scenes for the
//! preview, the export and `devcast frame`/`sheet`.

use dc_core::{CursorTracks, OutputTime, PixelSize, Project, StateError, TimeDelta, TimeMap};
use dc_fx_crop::Crop;
use dc_fx_cursor::Cursor;
use dc_fx_zoom::{CameraTrack, ZoomEffect};
use dc_render::Scene;

/// The project's scene can not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvaluateError {
    #[error(transparent)]
    State(#[from] StateError),

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
}

impl<'p> Evaluator<'p> {
    /// `cursors` — the cursor telemetry of the recordings on the track (decision 015); the
    /// caller reads it from the project folder.
    pub fn new(project: &'p Project, cursors: CursorTracks) -> Result<Self, EvaluateError> {
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
        Ok(Self {
            project,
            map,
            canvas,
            camera,
            cursor: dc_fx_cursor::Prepared::new(project, cursor.settings, cursors),
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
        scene
    }
}
