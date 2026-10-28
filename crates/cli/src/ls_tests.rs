use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use dc_project::ProjectDir;

use crate::{Outcome, registry::registry, run};

pub(crate) struct Fixture {
    _tmp: tempfile::TempDir,
    pub(crate) dir: ProjectDir,
}

impl Fixture {
    /// Two recordings, 20 s and 10 s.
    pub(crate) fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("Урок 2.dcproj")).unwrap();
        let mut project = Project::default();
        for (name, secs) in [("a", 20.0), ("b", 10.0)] {
            let duration = TimeDelta::from_secs_f64(secs);
            let id = project.clip_ids.allocate();
            project.clips.push(Clip {
                id,
                media: Media {
                    path: format!("media/{name}.mp4"),
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
            });
        }
        dir.save(&project).unwrap();
        Self { _tmp: tmp, dir }
    }

    pub(crate) fn run(&self, line: &str) -> Outcome {
        let args = line.split(' ').map(str::to_owned).collect();
        run(args, self.dir.root(), &registry().unwrap())
    }

    fn ok(&self, line: &str) -> String {
        let out = self.run(line);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        out.stdout.trim_end().to_owned()
    }
}

#[test]
fn ls_shows_objects_in_output_time_sorted_and_filtered() {
    let f = Fixture::new();
    f.ok("zoom add --at 12 --duration 3 --scale 2");
    f.ok("zoom add --at 2 --duration 1");
    f.ok("cut 0-1");
    assert_eq!(
        f.ok("ls zooms --fields scale"),
        "z2  0:01.000–0:02.000  scale=1.5\nz1  0:11.000–0:14.000  scale=2.0"
    );
    assert_eq!(
        f.ok("ls zooms --from 5 --fields mode"),
        "z1  0:11.000–0:14.000  mode=follow"
    );
    assert_eq!(
        f.ok("ls zooms --limit 1 --fields scale"),
        "z2  0:01.000–0:02.000  scale=1.5\nпоказано 1–1 из 2; дальше: devcast ls zooms --offset 1"
    );
    assert_eq!(f.ok("ls zooms --to 1"), "zooms: пусто (всего 0)");
    f.ok("cut 1-2");
    assert!(f.ok("ls zooms").starts_with("z1  0:10.000–0:13.000"));
    assert!(
        f.ok("ls zooms")
            .ends_with("z2  вырезан  focus={\"x\":0.5,\"y\":0.5}  mode=follow  scale=1.5"),
        "cut out objects stay listed, last"
    );
}

#[test]
fn ls_clips_and_info_summarise_the_project() {
    let f = Fixture::new();
    f.ok("cut 0-1");
    f.ok("zoom add --at 20 --duration 2");
    assert_eq!(
        f.ok("ls clips"),
        "c1  0:00.000–0:19.000  cuts=1  media=media/a.mp4  speeds=0  trim=0:00.000–0:20.000\n\
         c2  0:19.000–0:29.000  cuts=0  media=media/b.mp4  speeds=0  trim=0:00.000–0:10.000"
    );
    assert_eq!(
        f.ok("info"),
        "Урок 2.dcproj: длительность 0:29.000, всё сохранено\nсписки: clips 2, zooms 1; подробно: devcast ls <список>"
    );
    let json: serde_json::Value = serde_json::from_str(&f.ok("ls zooms --json")).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["items"][0]["from"], 20.0);
    assert_eq!(json["items"][0]["media"], serde_json::Value::Null);
}

#[test]
fn ls_errors_name_what_exists() {
    let f = Fixture::new();
    let out = f.run("ls zoomz");
    assert_eq!(out.code, 2);
    assert_eq!(out.stderr, "нет списка «zoomz»; есть: clips, zooms\n");
    f.ok("zoom add --at 1 --duration 1");
    let out = f.run("ls zooms --fields scal");
    assert_eq!(out.code, 2);
    assert_eq!(out.stderr, "нет поля «scal»; есть: focus, mode, scale\n");
    assert_eq!(f.run("ls zooms --from soon").code, 2);
}
