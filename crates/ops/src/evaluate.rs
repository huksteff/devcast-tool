//! `evaluate(project, t) -> Scene` of the whole project (ARCHITECTURE, decision 4; decision 015):
//! every effect adds its part to the scene in a fixed order. The one source of scenes for the
//! preview, the export and `devcast frame`/`sheet`.

use dc_core::{OutputTime, PixelSize, Project, StateError, TimeDelta, TimeMap};
use dc_fx_crop::Crop;
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
}

impl<'p> Evaluator<'p> {
    pub fn new(project: &'p Project) -> Result<Self, EvaluateError> {
        let crop = project.state::<Crop>()?;
        let canvas = dc_fx_crop::canvas(project, &crop.settings).ok_or(EvaluateError::Empty)?;
        let map = TimeMap::build(project);
        let camera = CameraTrack::new(&project.state::<ZoomEffect>()?, &map, canvas);
        Ok(Self {
            project,
            map,
            canvas,
            camera,
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
        scene
    }
}
