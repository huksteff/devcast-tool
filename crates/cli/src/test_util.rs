//! Test projects and test commands shared by the tests of this crate.

use std::path::{Path, PathBuf};

use dc_capture::input::{
    CURSOR_FILE, CursorEvent, KEYS_FILE, KeyEvent, Record, RecordWriter, Timed,
};
use dc_capture::{HostTime, Manifest, Track, VIDEO_TRACK};
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
        audio: Default::default(),
    }
}

pub(crate) fn run_line(line: &str, cwd: &Path, registry: &Registry) -> Outcome {
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
    const NAME: &'static str = "test add";
    const ABOUT: &'static str = "Добавить клип";
    const EXAMPLE: &'static str = "test add --secs 2";
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
    const NAME: &'static str = "test rm";
    const ABOUT: &'static str = "Удалить клип";
    const EXAMPLE: &'static str = "test rm c1";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

/// An empty project `Мои видео/Урок 1.dcproj` with the test commands `test add` and `test rm`.
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
        Self::of(&[("media/a.mp4", 20.0), ("media/b.mp4", 10.0)])
    }

    fn of(clips: &[(&str, f64)]) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("Урок 2.dcproj")).unwrap();
        let mut project = Project::default();
        for (path, secs) in clips {
            let id = project.clip_ids.allocate();
            project.clips.push(recording(id, (*path).to_owned(), *secs));
        }
        dir.save(&project).unwrap();
        Self { _tmp: tmp, dir }
    }

    /// Recordings `media/recording-N/video.mp4` (from 1) of these lengths in seconds, each with
    /// a manifest (video from host time 0) and empty telemetry; fill it with [`Self::cursor`]
    /// and [`Self::keys`].
    pub(crate) fn with_telemetry(lengths: &[f64]) -> Self {
        let paths: Vec<String> = (1..=lengths.len())
            .map(|n| format!("media/recording-{n}/video.mp4"))
            .collect();
        let clips: Vec<(&str, f64)> = paths
            .iter()
            .map(String::as_str)
            .zip(lengths.iter().copied())
            .collect();
        let this = Self::of(&clips);
        for (n, secs) in (1..).zip(lengths) {
            let folder = this.folder(n);
            std::fs::create_dir_all(&folder).unwrap();
            let mut video = Track::new(VIDEO_TRACK, 60, HostTime::from_micros(0));
            video.file = Some(video.final_file());
            video.length = (secs * 60.0) as i64;
            video.size = Some((1920, 1080));
            Manifest::new(vec![video]).write(&folder).unwrap();
            this.cursor(n, &[]);
            this.keys(n, &[]);
        }
        this
    }

    fn folder(&self, n: usize) -> PathBuf {
        self.dir.media_dir().join(format!("recording-{n}"))
    }

    /// Replaces `cursor.bin` of recording `n` with `events` at source seconds.
    pub(crate) fn cursor(&self, n: usize, events: &[(f64, CursorEvent)]) {
        write(&self.folder(n).join(CURSOR_FILE), events);
    }

    /// Replaces `keys.bin` of recording `n` with `events` at source seconds.
    pub(crate) fn keys(&self, n: usize, events: &[(f64, KeyEvent)]) {
        write(&self.folder(n).join(KEYS_FILE), events);
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

fn write<E: Copy>(path: &Path, events: &[(f64, E)])
where
    Timed<E>: Record,
{
    let mut writer = RecordWriter::create(path).unwrap();
    for &(secs, event) in events {
        let at = HostTime::from_micros((secs * 1e6).round() as i64);
        writer.write(&Timed { at, event }).unwrap();
    }
    writer.flush().unwrap();
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

/// Runs the FFmpeg command line from `.deps` with `args` after `-v error -y`.
pub(crate) fn ffmpeg(args: &[&str]) {
    let exe = Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
    let status = std::process::Command::new(exe)
        .args(["-v", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg {args:?}");
}

/// A 320×240 `testsrc2` video `secs` long at 30 fps.
pub(crate) fn test_video(path: &Path, secs: f64) {
    let source = "testsrc2=size=320x240:rate=30";
    let secs = secs.to_string();
    let output = path.to_str().unwrap();
    let video_codec = ["-c:v", "libx264", "-pix_fmt", "yuv420p"];
    ffmpeg(
        &[
            &["-f", "lavfi", "-i", source, "-t", &secs][..],
            &video_codec,
            &[output],
        ]
        .concat(),
    );
}
