use std::path::Path;

use crate::{Outcome, run};
use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, ClipId, Command, CommandError, Diff, Op, OpInfo, Project, Range, SourceTime};
use dc_core::{TimeDelta, history::LIMIT};
use dc_ops::Registry;
use dc_project::{HistoryLog, ProjectDir};
use schemars::JsonSchema;
use serde::Deserialize;

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
        let duration = TimeDelta::from_secs_f64(self.secs);
        let trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + duration)
            .map_err(|e| CommandError::new(e.to_string()))?;
        project.clips.push(Clip {
            id,
            media: Media {
                path: "m.mp4".to_owned(),
                kind: MediaKind::Imported,
                duration,
            },
            trim,
            cuts: Vec::new(),
            speeds: Vec::new(),
        });
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

pub(crate) struct Fixture {
    pub(crate) _tmp: tempfile::TempDir,
    pub(crate) cwd: std::path::PathBuf,
    pub(crate) dir: ProjectDir,
    registry: Registry,
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
        self.run_in(line, &self.cwd)
    }

    fn run_in(&self, line: &str, cwd: &Path) -> Outcome {
        let args = line.split(' ').map(str::to_owned).collect();
        run(args, cwd, &self.registry)
    }

    pub(crate) fn clips(&self) -> usize {
        self.dir.load().unwrap().clips.len()
    }
}

pub(crate) fn done(stdout: &str) -> Outcome {
    Outcome {
        code: 0,
        stdout: format!("{stdout}\n"),
        stderr: String::new(),
    }
}

#[test]
fn op_is_applied_saved_and_logged() {
    let f = Fixture::new();
    assert_eq!(
        f.run("clip add --secs 2"),
        done("клипы +1 c1; длительность 0:00.000→0:02.000")
    );
    assert_eq!(f.clips(), 1, "saved to project.json right away");
    let project = f.dir.load().unwrap();
    let history = HistoryLog::new(&f.dir).load(&project, LIMIT).unwrap();
    assert_eq!(history.history.changes().len(), 1);
    assert!(history.warnings.is_empty());
}

#[test]
fn global_flags_work_anywhere_and_dry_run_writes_nothing() {
    let f = Fixture::new();
    let out = f.run("clip --dry-run add --secs 2");
    assert_eq!(
        out,
        done("dry-run, проект не изменён: клипы +1 c1; длительность 0:00.000→0:02.000")
    );
    assert_eq!(f.clips(), 0);
    assert!(!f.dir.file("history.jsonl").exists());
    let out = f.run("clip add --json --secs 1.5");
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"changed": true, "dry_run": false, "diff": "клипы +1 c1", "duration": [0.0, 1.5]})
    );
}

#[test]
fn unsaved_editor_state_stays_unsaved() {
    let f = Fixture::new();
    f.dir.save_session(&Project::default()).unwrap();
    assert_eq!(f.run("clip add --secs 1").code, 0);
    assert_eq!(f.clips(), 0, "project.json untouched");
    assert_eq!(f.dir.load_session().unwrap().unwrap().clips.len(), 1);
}

#[test]
fn errors_go_to_stderr_with_exit_codes() {
    let f = Fixture::new();
    let out = f.run("clip rn c1");
    assert_eq!(out.code, 2);
    assert!(out.stdout.is_empty());
    assert!(
        out.stderr.contains("похоже, нужна «clip rm»"),
        "{}",
        out.stderr
    );

    let out = f.run("clip rm c9");
    assert_eq!(out.code, 1);
    assert_eq!(
        out.stderr,
        "c9 не найден (нет ни одного клипа); список: devcast ls clips\n"
    );

    let out = f.run("clip add --secs nope --json");
    assert_eq!(out.code, 2);
    let json: serde_json::Value = serde_json::from_str(&out.stderr).unwrap();
    assert_eq!(json["code"], 2);
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("devcast help clip add")
    );

    let empty = f._tmp.path();
    let out = f.run_in("clip add --secs 1", empty);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("нет проекта *.dcproj"),
        "{}",
        out.stderr
    );
}

#[test]
fn project_flag_and_folder_inside_project() {
    let f = Fixture::new();
    let other = f._tmp.path();
    let args = vec![
        "clip".to_owned(),
        "add".to_owned(),
        "--secs".to_owned(),
        "1".to_owned(),
        "--project".to_owned(),
        "Мои видео/Урок 1.dcproj".to_owned(),
    ];
    assert_eq!(run(args, other, &f.registry).code, 0);
    assert_eq!(f.run_in("clip rm c1", &f.dir.media_dir()).code, 0);
    assert_eq!(f.clips(), 0);
}

#[test]
fn help_lists_every_command_on_one_line() {
    let f = Fixture::new();
    let out = f.run("help");
    assert_eq!(out.code, 0);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "help      Список команд; с именем команды — её аргументы и пример",
            "apply     Применить план из JSON-файла: все шаги — один шаг отмены, всё или ничего",
            "undo      Отменить последние шаги",
            "redo      Вернуть последний отменённый шаг",
            "history   Шаги, которые отменит undo и вернёт redo, новые сверху",
            "save      Сохранить несохранённые изменения редактора в project.json",
            "clip add  Добавить клип",
            "clip rm   Удалить клип",
            "Подробнее: devcast help <команда>; всё со схемами: devcast help --json",
        ]
    );
}

#[test]
fn help_for_one_command_shows_args_and_example() {
    let f = Fixture::new();
    assert_eq!(
        f.run("help clip rm"),
        done(
            "clip rm — Удалить клип
пример: devcast clip rm c1
  <id>: id c<число>, обязательный — Короткий id клипа: c<число>"
        )
    );
    assert_eq!(
        f.run("help clip add"),
        done(
            "clip add — Добавить клип
пример: devcast clip add --secs 2
  --secs: число, обязательный — Длина клипа, с"
        )
    );
    assert_eq!(
        f.run("help help"),
        done(
            "help — Список команд; с именем команды — её аргументы и пример
пример: devcast help zoom add
  <command>: текст — Команда из нескольких слов, например zoom add"
        )
    );
    let out = f.run("help clip rn");
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("похоже, нужна «clip rm»"),
        "{}",
        out.stderr
    );
}

#[test]
fn help_json_has_schemas_of_ops_project_and_effects() {
    let f = Fixture::new();
    let out = f.run("help clip add --json");
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["example"], "devcast clip add --secs 2");
    assert_eq!(
        json["args"],
        serde_json::json!([{"name": "--secs", "kind": "число", "required": true, "about": "Длина клипа, с"}])
    );
    assert_eq!(json["schema"]["additionalProperties"], false);

    let out = f.run("help --json");
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    let names: Vec<&str> = json["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "help", "apply", "undo", "redo", "history", "save", "clip add", "clip rm"
        ]
    );
    assert!(json["project"]["properties"]["clips"].is_object());
    assert!(json["effects"].is_object());
}
