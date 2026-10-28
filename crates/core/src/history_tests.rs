use super::*;
use crate::effect::Effect;
use crate::project::{Media, MediaKind};
use crate::time::{Range, SourceTime, TimeDelta};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
struct Items {
    items: Vec<u32>,
}

struct Test;
impl Effect for Test {
    const ID: &'static str = "test";
    const TITLE: &'static str = "Тест";
    type State = Items;
}

#[derive(Debug)]
struct Push(u32);
impl Command for Push {
    fn name(&self) -> &'static str {
        "test push"
    }
    fn apply(&self, project: &mut Project) -> Result<String, CommandError> {
        let mut state = project.state::<Test>()?;
        state.items.push(self.0);
        project.set_state::<Test>(&state)?;
        Ok(format!("+{}", self.0))
    }
}

#[derive(Debug)]
struct Fail;
impl Command for Fail {
    fn name(&self) -> &'static str {
        "test fail"
    }
    fn apply(&self, _: &mut Project) -> Result<String, CommandError> {
        Err(CommandError::new("нельзя"))
    }
}

/// Adds a clip whose trim is longer than its media: breaks an invariant on purpose.
#[derive(Debug)]
struct BrokenClip;
impl Command for BrokenClip {
    fn name(&self) -> &'static str {
        "test broken"
    }
    fn apply(&self, project: &mut Project) -> Result<String, CommandError> {
        let id = project.clip_ids.allocate();
        let end = SourceTime::from_secs_f64(5.0);
        project.clips.push(Clip {
            id,
            media: Media {
                path: "m.mp4".to_owned(),
                kind: MediaKind::Imported,
                duration: TimeDelta::from_secs_f64(1.0),
            },
            trim: Range::new(SourceTime::ZERO, end)
                .map_err(|e| CommandError::new(e.to_string()))?,
            cuts: Vec::new(),
            speeds: Vec::new(),
        });
        Ok("+clip".to_owned())
    }
}

fn items(p: &Project) -> Vec<u32> {
    p.state::<Test>().unwrap().items
}

#[test]
fn undo_and_redo_restore_exact_states() {
    let mut p = Project::default();
    let mut h = History::default();
    assert_eq!(h.apply(&mut p, &Push(1)).unwrap(), "+1");
    h.apply(&mut p, &Push(2)).unwrap();
    assert_eq!(items(&p), vec![1, 2]);
    assert_eq!(h.undo(&mut p).unwrap().summary, "+2");
    assert_eq!(items(&p), vec![1]);
    h.undo(&mut p);
    assert!(p.effects.is_empty(), "back to no state at all");
    h.redo(&mut p);
    h.redo(&mut p);
    assert_eq!(items(&p), vec![1, 2]);
    assert!(h.redo(&mut p).is_none());
}

#[test]
fn new_change_clears_redo() {
    let mut p = Project::default();
    let mut h = History::default();
    h.apply(&mut p, &Push(1)).unwrap();
    h.undo(&mut p);
    h.apply(&mut p, &Push(5)).unwrap();
    assert!(!h.can_redo());
    assert_eq!(items(&p), vec![5]);
}

#[test]
fn batch_is_one_step_and_all_or_nothing() {
    let mut p = Project::default();
    let mut h = History::default();
    assert_eq!(
        h.apply_batch(&mut p, "apply", &[&Push(1), &Push(2)])
            .unwrap(),
        "+1; +2"
    );
    assert_eq!(h.changes().len(), 1);
    let error = h
        .apply_batch(&mut p, "apply", &[&Push(3), &Fail])
        .unwrap_err();
    assert_eq!(error.to_string(), "нельзя");
    assert_eq!(
        items(&p),
        vec![1, 2],
        "failed batch leaves the project untouched"
    );
    assert_eq!(h.changes().len(), 1);
}

#[test]
fn command_breaking_invariants_is_rolled_back() {
    let mut p = Project::default();
    let mut h = History::default();
    let error = h.apply(&mut p, &BrokenClip).unwrap_err().to_string();
    assert!(
        error.contains("внутренняя ошибка команды «test broken»"),
        "{error}"
    );
    assert!(p.clips.is_empty());
    assert!(!h.can_undo());
}

#[test]
fn no_op_is_not_recorded() {
    #[derive(Debug)]
    struct Nothing;
    impl Command for Nothing {
        fn name(&self) -> &'static str {
            "nothing"
        }
        fn apply(&self, _: &mut Project) -> Result<String, CommandError> {
            Ok("без изменений".to_owned())
        }
    }
    let mut p = Project::default();
    let mut h = History::default();
    h.apply(&mut p, &Nothing).unwrap();
    assert!(!h.can_undo());
}

#[test]
fn limit_drops_oldest_steps() {
    let mut p = Project::default();
    let mut h = History::new(2);
    for i in 0..5 {
        h.apply(&mut p, &Push(i)).unwrap();
    }
    assert_eq!(h.changes().len(), 2);
    assert_eq!(h.changes()[0].summary, "+3");
}

#[test]
fn changes_survive_json() {
    let mut p = Project::default();
    let mut h = History::default();
    h.apply(&mut p, &Push(7)).unwrap();
    let json = serde_json::to_string(&h.changes()[0]).unwrap();
    let back: Change = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, &h.changes()[0]);
    let mut restored = History::from_stacks(vec![back], Vec::new(), 10);
    restored.undo(&mut p);
    assert!(p.effects.is_empty());
}
