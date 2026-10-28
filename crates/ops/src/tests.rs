use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, ClipId, Command, CommandError, Diff, History, Op, OpInfo, Project, Range, SourceTime,
    TimeDelta,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Registry, Step, apply, parse_args, parse_plan};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Note {
    text: String,
    #[serde(default)]
    count: u32,
    #[serde(default)]
    loud: bool,
}

impl Command for Note {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        if self.count > 9 {
            return Err(CommandError::new("count не больше 9"));
        }
        let value = json!({"text": self.text, "count": self.count, "loud": self.loud});
        project.effects.insert("note".to_owned(), value);
        Ok(Diff::note(format!("note={}", self.text)))
    }
}

impl Op for Note {
    const NAME: &'static str = "note set";
    const ABOUT: &'static str = "Записать заметку";
    const EXAMPLE: &'static str = "note set привет --count 2";
    const POSITIONAL: &'static [&'static str] = &["text"];
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Loose {}

impl Command for Loose {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn apply(&self, _: &mut Project) -> Result<Diff, CommandError> {
        Ok(Diff::default())
    }
}

impl Op for Loose {
    const NAME: &'static str = "Loose";
    const ABOUT: &'static str = "";
    const EXAMPLE: &'static str = "other";
    const POSITIONAL: &'static [&'static str] = &["missing"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClipAdd {
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

fn registry() -> Registry {
    Registry::new(
        vec![
            OpInfo::of::<Note>(),
            OpInfo::of::<ClipAdd>(),
            OpInfo::of::<ClipRm>(),
        ],
        Vec::new(),
    )
    .unwrap()
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn registry_rejects_ops_breaking_the_rules() {
    let error = Registry::new(
        vec![
            OpInfo::of::<Loose>(),
            OpInfo::of::<Note>(),
            OpInfo::of::<Note>(),
        ],
        Vec::new(),
    )
    .unwrap_err();
    for expected in [
        "строчных",
        "одна непустая",
        "начинаться",
        "deny_unknown_fields",
        "«missing»",
        "дважды",
    ] {
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn unknown_op_suggests_the_closest() {
    let error = registry().find("note sett").unwrap_err().to_string();
    assert!(error.contains("похоже, нужна «note set»"), "{error}");
    let error = registry().find("export").unwrap_err().to_string();
    assert!(
        !error.contains("похоже") && error.contains("devcast help"),
        "{error}"
    );
}

#[test]
fn command_line_maps_to_fields() {
    let registry = registry();
    let op = registry.find("note set").unwrap();
    let parsed = parse_args(op, &args(&["123", "--count", "2", "--loud"])).unwrap();
    assert_eq!(
        Value::Object(parsed),
        json!({"text": "123", "count": 2, "loud": true})
    );
    let parsed = parse_args(op, &args(&["--text=a b", "--count=3"])).unwrap();
    assert_eq!(Value::Object(parsed), json!({"text": "a b", "count": 3}));
    let error = parse_args(op, &args(&["a", "b"])).unwrap_err().to_string();
    assert!(
        error.contains("лишний аргумент «b»") && error.contains("devcast note set"),
        "{error}"
    );
    let error = parse_args(op, &args(&["a", "--text", "b"]))
        .unwrap_err()
        .to_string();
    assert!(error.contains("дважды"), "{error}");
}

#[test]
fn bad_arguments_point_to_help() {
    let mut fields = serde_json::Map::new();
    fields.insert("txt".to_owned(), json!("x"));
    let error = registry()
        .command("note set", fields)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("devcast help note set") && error.contains("txt"),
        "{error}"
    );
}

#[test]
fn plan_is_one_undo_step_and_dry_run_changes_nothing() {
    let registry = registry();
    let steps =
        parse_plan(r#"[{"op":"note set","text":"a"},{"op":"note set","text":"b","count":1}]"#)
            .unwrap();
    let (mut project, mut history) = (Project::default(), History::default());
    let dry = apply(&registry, &mut project, &mut history, &steps, true).unwrap();
    assert!(dry.changed && dry.dry_run);
    assert_eq!(
        dry.to_string(),
        "dry-run, проект не изменён: note=a; note=b"
    );
    assert_eq!(project, Project::default());
    assert!(!history.can_undo());
    let done = apply(&registry, &mut project, &mut history, &steps, false).unwrap();
    assert!(done.changed && history.changes().len() == 1 && history.changes()[0].name == "apply");
    let again = apply(&registry, &mut project, &mut history, &steps[1..], false).unwrap();
    assert!(
        !again.changed && history.changes().len() == 1,
        "no-op is not recorded"
    );
    assert_eq!(again.to_string(), "без изменений");
}

#[test]
fn one_line_answer_merges_ids_and_shows_duration() {
    let registry = registry();
    let steps = parse_plan(
        r#"[{"op":"clip add","secs":2},{"op":"clip add","secs":1.5},{"op":"clip add","secs":2}]"#,
    )
    .unwrap();
    let (mut project, mut history) = (Project::default(), History::default());
    let done = apply(&registry, &mut project, &mut history, &steps, false).unwrap();
    assert_eq!(
        done.to_string(),
        "клипы +3 c1..c3; длительность 0:00.000→0:05.500"
    );
    assert_eq!(history.changes()[0].summary, "клипы +3 c1..c3");
    let rm = vec![Step::new(
        "clip rm",
        json!({"id": "c2"}).as_object().unwrap().clone(),
    )];
    let done = apply(&registry, &mut project, &mut history, &rm, false).unwrap();
    assert_eq!(
        done.to_string(),
        "клипы -1 c2; длительность 0:05.500→0:04.000"
    );
}

#[test]
fn missing_id_lists_existing_and_how_to_list() {
    let registry = registry();
    let steps = parse_plan(r#"[{"op":"clip add","secs":1},{"op":"clip add","secs":1}]"#).unwrap();
    let (mut project, mut history) = (Project::default(), History::default());
    apply(&registry, &mut project, &mut history, &steps, false).unwrap();
    let rm = vec![Step::new(
        "clip rm",
        json!({"id": "c9"}).as_object().unwrap().clone(),
    )];
    let error = apply(&registry, &mut project, &mut history, &rm, false)
        .unwrap_err()
        .to_string();
    assert_eq!(error, "c9 не найден (есть c1,c2); список: devcast ls clips");
}

#[test]
fn failing_step_names_its_number_and_changes_nothing() {
    let registry = registry();
    let steps = vec![
        Step::new(
            "note set",
            json!({"text": "a"}).as_object().unwrap().clone(),
        ),
        Step::new(
            "note set",
            json!({"text": "b", "count": 10})
                .as_object()
                .unwrap()
                .clone(),
        ),
    ];
    let (mut project, mut history) = (Project::default(), History::default());
    let error = apply(&registry, &mut project, &mut history, &steps, false)
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with("шаг 2 (note set): count не больше 9"),
        "{error}"
    );
    assert_eq!(project, Project::default());
}

#[test]
fn malformed_plans_explain_the_format() {
    for (text, expected) in [
        ("{", "не читается"),
        (r#"{"op":"x"}"#, "одиночный"),
        ("[]", "пуст"),
        (r#"[{"text":"a"}]"#, "шаг 1: нет поля"),
    ] {
        let error = parse_plan(text).unwrap_err().to_string();
        assert!(error.contains(expected), "{text}: {error}");
    }
}
