use dc_core::{Command, CommandError, Diff, Op, Project, Rect};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Crop;
use crate::state::{Aspect, MIN_SIZE};

/// Crops a clip.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CropClip {
    pub clip: dc_core::ClipId,
    /// Visible part as fractions of the source frame: inside 0..1, width and height at least 0.05.
    pub rect: Rect,
}

impl Command for CropClip {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        if project.clip(self.clip).is_none() {
            return Err(CommandError::not_found(
                self.clip,
                project.clips.iter().map(|c| c.id),
            ));
        }
        let rect = self.rect;
        check(rect)?;
        let mut state = project.state::<Crop>()?;
        let note = if rect == Rect::FULL {
            state.clips.remove(&self.clip);
            format!("кроп {} снят", self.clip)
        } else {
            state.clips.insert(self.clip, rect);
            format!(
                "кроп {} {},{},{},{}",
                self.clip,
                rect.x(),
                rect.y(),
                rect.w(),
                rect.h()
            )
        };
        project.set_state::<Crop>(&state)?;
        Ok(Diff::note(note))
    }
}

fn check(rect: Rect) -> Result<(), CommandError> {
    if rect.w() < MIN_SIZE || rect.h() < MIN_SIZE {
        return Err(CommandError::new(format!(
            "ширина и высота кропа — не меньше {MIN_SIZE} (5% кадра); сейчас {}x{}",
            rect.w(),
            rect.h()
        )));
    }
    if rect.x() < 0.0 || rect.y() < 0.0 || rect.x() + rect.w() > 1.0 || rect.y() + rect.h() > 1.0 {
        return Err(CommandError::new(format!(
            "кроп должен лежать внутри кадра: x, y ≥ 0, x+w ≤ 1, y+h ≤ 1; сейчас {},{},{},{}. Весь кадр: 0,0,1,1",
            rect.x(),
            rect.y(),
            rect.w(),
            rect.h()
        )));
    }
    Ok(())
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
}
