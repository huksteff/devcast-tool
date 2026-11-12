//! `evaluate(project, t) -> Scene` of the whole project (ARCHITECTURE, decision 4; decision 015):
//! every effect adds its part to the scene in the order of the list of pictures
//! (`devcast.rs`, `LOOKS`). The one source of scenes for the preview, the export and
//! `devcast frame`/`sheet`. It knows no effect's types: each one prepares itself and draws
//! through the seam `dc_render::look`.

use dc_core::TimeMap;
use dc_core::{CursorTracks, KeyTracks, OutputTime, PixelSize, Project, StateError, TimeDelta};
use dc_render::look::{Frame, Inputs, Look, LookError};
use dc_render::{RenderError, Scene, TextMeasure};

use crate::devcast::{LOOKS, canvas};

/// The project's scene can not be built.
#[derive(Debug, thiserror::Error)]
pub enum EvaluateError {
    #[error(transparent)]
    State(#[from] StateError),

    #[error(transparent)]
    Text(#[from] RenderError),

    /// An effect's own reason; its text says what to do.
    #[error(transparent)]
    Effect(Box<dyn std::error::Error + Send + Sync>),

    #[error("в проекте нет клипов: добавь видео (devcast clip add <видео>) и попробуй снова")]
    Empty,
}

impl From<LookError> for EvaluateError {
    fn from(error: LookError) -> Self {
        match error {
            LookError::State(error) => Self::State(error),
            LookError::Text(error) => Self::Text(error),
            LookError::Effect(error) => Self::Effect(error),
        }
    }
}

/// A project ready to be evaluated at any output moment: the time map and the effects are
/// prepared once, not per frame.
#[derive(Debug)]
pub struct Evaluator<'p> {
    project: &'p Project,
    map: TimeMap,
    canvas: PixelSize,
    /// The effects prepared, in the order of `LOOKS`.
    looks: Vec<Box<dyn Look>>,
}

impl<'p> Evaluator<'p> {
    /// `cursors` and `keys` — the telemetry of the recordings on the track (decision 015); the
    /// caller reads it from the project folder. `measure` holds the fonts the frames are drawn
    /// with: texts are broken into lines here (decision 016).
    pub fn new(
        project: &'p Project,
        cursors: CursorTracks,
        keys: KeyTracks,
        measure: &mut TextMeasure,
    ) -> Result<Self, EvaluateError> {
        let canvas = canvas(project)?.ok_or(EvaluateError::Empty)?;
        let map = TimeMap::build(project);
        let mut inputs = Inputs {
            project,
            map: &map,
            canvas,
            cursors: &cursors,
            keys: &keys,
            measure,
        };
        let looks = LOOKS
            .iter()
            .map(|look| look.prepare(&mut inputs))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            project,
            map,
            canvas,
            looks,
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
        let mut frame = Frame {
            project: self.project,
            map: &self.map,
            t,
            scene: &mut scene,
            pointer: None,
        };
        for look in &self.looks {
            look.draw(&mut frame);
        }
        scene
    }
}
