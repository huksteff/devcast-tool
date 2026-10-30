use dc_core::project::crop_problem;
use dc_core::{Command, CommandError, Diff, Op, Project, Rect};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Crop;
use crate::state::Aspect;

/// Crops a clip.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CropClip {
    /// Clip to crop.
    pub clip: dc_core::ClipId,
    /// Visible part as fractions of the source frame: inside 0..1, width and height at least 0.05.
    pub rect: Rect,
}

impl Command for CropClip {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        if let Some(problem) = crop_problem(self.rect) {
            return Err(CommandError::new(problem));
        }
        let ids: Vec<_> = project.clips.iter().map(|c| c.id).collect();
        let clip = project
            .clip_mut(self.clip)
            .ok_or_else(|| CommandError::not_found(self.clip, ids))?;
        clip.crop = self.rect;
        let r = self.rect;
        Ok(Diff::note(if r == Rect::FULL {
            format!("кроп {} снят", self.clip)
        } else {
            format!("кроп {} {},{},{},{}", self.clip, r.x(), r.y(), r.w(), r.h())
        }))
    }
}

impl Op for CropClip {
    const NAME: &'static str = "crop";
    const ABOUT: &'static str = "Кроп клипа: x,y,w,h в долях исходника, 0,0,1,1 снимает кроп";
    const EXAMPLE: &'static str = "crop c1 0.1,0,0.8,1";
    const POSITIONAL: &'static [&'static str] = &["clip", "rect"];
}

/// Sets the aspect of the output frame.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetAspect {
    /// Aspect of the output frame.
    pub aspect: Aspect,
}

impl Command for SetAspect {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Crop>()?;
        state.settings.aspect = self.aspect;
        project.set_state::<Crop>(&state)?;
        Ok(Diff::note(format!("формат {}", self.aspect)))
    }
}

impl Op for SetAspect {
    const NAME: &'static str = "aspect";
    const ABOUT: &'static str = "Формат выхода: free, source, 16:9, 16:10 или 4:3";
    const EXAMPLE: &'static str = "aspect 16:9";
    const POSITIONAL: &'static [&'static str] = &["aspect"];
    const SETTINGS: Option<&'static str> = Some("crop");
}
