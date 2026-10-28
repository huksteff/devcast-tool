//! Clips on the magnetic main track: add, remove, reorder. The track has no gaps, so adding,
//! removing or moving a clip only changes `project.clips`.

use dc_core::project::{Clip, Media};
use dc_core::{ClipId, Command, CommandError, Diff, Op, Project, Range, Rect, SourceTime};
use schemars::JsonSchema;
use serde::Deserialize;

fn index_of(project: &Project, id: ClipId) -> Result<usize, CommandError> {
    project
        .clips
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(|| CommandError::not_found(id, project.clips.iter().map(|c| c.id)))
}

/// Adds a clip of the whole `media` at the end of the track. Not an [`Op`]: the media file must
/// be in the project's `media/` first, so the CLI copies and probes it and then applies this.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipAdd {
    pub media: Media,
}

impl ClipAdd {
    pub const NAME: &'static str = "clip add";
}

impl Command for ClipAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let end = SourceTime::ZERO + self.media.duration;
        let trim = Range::new(SourceTime::ZERO, end).map_err(|_| {
            CommandError::new(format!(
                "у видео {} отрицательная длина: файл повреждён, перекодируй его",
                self.media.path
            ))
        })?;
        let id = project.clip_ids.allocate();
        project.clips.push(Clip {
            id,
            media: self.media.clone(),
            trim,
            cuts: Vec::new(),
            speeds: Vec::new(),
            crop: Rect::FULL,
        });
        Ok(Diff::added(id))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClipRm {
    /// Клип, например c2.
    pub id: ClipId,
}

impl Command for ClipRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let index = index_of(project, self.id)?;
        if project.clips.len() == 1 {
            return Err(CommandError::new(format!(
                "{} — последний клип, без него проект пуст. Добавь другой клип или вырежи часть: devcast cut",
                self.id
            )));
        }
        project.clips.remove(index);
        Ok(Diff::removed(self.id))
    }
}

impl Op for ClipRm {
    const NAME: &'static str = "clip rm";
    const ABOUT: &'static str = "Удалить клип с главной дорожки; следующие сдвигаются без дыры";
    const EXAMPLE: &'static str = "clip rm c2";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClipMove {
    /// Клип, например c2.
    pub id: ClipId,
    /// Новое место на дорожке, с 1.
    pub to: usize,
}

impl Command for ClipMove {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let index = index_of(project, self.id)?;
        let count = project.clips.len();
        if !(1..=count).contains(&self.to) {
            return Err(CommandError::new(format!(
                "место {} вне дорожки: клипов {count}, место 1–{count}",
                self.to
            )));
        }
        if index + 1 == self.to {
            return Ok(Diff::default());
        }
        let clip = project.clips.remove(index);
        project.clips.insert(self.to - 1, clip);
        Ok(Diff::changed(self.id))
    }
}

impl Op for ClipMove {
    const NAME: &'static str = "clip move";
    const ABOUT: &'static str = "Переставить клип на другое место главной дорожки";
    const EXAMPLE: &'static str = "clip move c2 --to 1";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[cfg(test)]
#[path = "clip_tests.rs"]
mod tests;
