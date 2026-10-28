//! Test projects and test commands shared by the tests of this crate.

use std::path::{Path, PathBuf};

use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, ClipId, Command, CommandError, Diff, Op, OpInfo, PixelSize, Project, Range};
use dc_core::{Rect, SourceTime, TimeDelta};
use dc_ops::Registry;
use dc_project::ProjectDir;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{Outcome, registry::registry, run};

/// A clip of the whole recording `path`, `secs` long.
fn recording(id: ClipId, path: String, secs: f64) -> Clip {
    let duration = TimeDelta::from_secs_f64(secs);
    Clip {
        id,
        media: Media {
            path,
            kind: MediaKind::Recording,
            duration,
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
    }
}

fn run_line(line: &str, cwd: &Path, registry: &Registry) -> Outcome {
    let args = line.split(' ').map(str::to_owned).collect();
    run(args, cwd, registry)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClipAdd {
    /// Длина клипа, с
    secs: f64,
}

impl Command for ClipAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let id = project.clip_ids.allocate();
        project
            .clips
            .push(recording(id, "m.mp4".to_owned(), self.secs));
        Ok(Diff::added(id))
    }
}

impl Op for ClipAdd {
    const NAME: &'static str = "clip add";
    const ABOUT: &'static str = "Добавить клип";
    const EXAMPLE: &'static str = "clip add --secs 2";
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClipRm {
    id: ClipId,
}

impl Command for ClipRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let ids = || project.clips.iter().map(|c| c.id);
        let index = ids()
            .position(|id| id == self.id)
            .ok_or_else(|| CommandError::not_found(self.id, ids()))?;
        project.clips.remove(index);
        Ok(Diff::removed(self.id))
    }
}

impl Op for ClipRm {
    const NAME: &'static str = "clip rm";
    const ABOUT: &'static str = "Удалить клип";
    const EXAMPLE: &'static str = "clip rm c1";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

/// An empty project `Мои видео/Урок 1.dcproj` with the test commands `clip add` and `clip rm`.
pub(crate) struct Fixture {
    pub(crate) _tmp: tempfile::TempDir,
    pub(crate) cwd: PathBuf,
    pub(crate) dir: ProjectDir,
    pub(crate) registry: Registry,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().join("Мои видео");
        std::fs::create_dir(&cwd).unwrap();
        let dir = ProjectDir::create(cwd.join("Урок 1.dcproj")).unwrap();
        let ops = vec![OpInfo::of::<ClipAdd>(), OpInfo::of::<ClipRm>()];
        Self {
            _tmp: tmp,
            cwd,
            dir,
            registry: Registry::new(ops, Vec::new()).unwrap(),
        }
    }

    pub(crate) fn run(&self, line: &str) -> Outcome {
        run_line(line, &self.cwd, &self.registry)
    }

    pub(crate) fn run_in(&self, line: &str, cwd: &Path) -> Outcome {
        run_line(line, cwd, &self.registry)
    }

    pub(crate) fn clips(&self) -> usize {
        self.dir.load().unwrap().clips.len()
    }
}

/// `Урок 2.dcproj` with two recordings, 20 s and 10 s, and the real command registry.
pub(crate) struct Recordings {
    _tmp: tempfile::TempDir,
    pub(crate) dir: ProjectDir,
}

impl Recordings {
    pub(crate) fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("Урок 2.dcproj")).unwrap();
        let mut project = Project::default();
        for (name, secs) in [("a", 20.0), ("b", 10.0)] {
            let id = project.clip_ids.allocate();
            project
                .clips
                .push(recording(id, format!("media/{name}.mp4"), secs));
        }
        dir.save(&project).unwrap();
        Self { _tmp: tmp, dir }
    }

    pub(crate) fn run(&self, line: &str) -> Outcome {
        run_line(line, self.dir.root(), &registry().unwrap())
    }

    pub(crate) fn ok(&self, line: &str) -> String {
        let out = self.run(line);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        out.stdout.trim_end().to_owned()
    }
}

pub(crate) fn done(stdout: &str) -> Outcome {
    Outcome {
        code: 0,
        stdout: format!(
            "{stdout}
"
        ),
        stderr: String::new(),
    }
}
