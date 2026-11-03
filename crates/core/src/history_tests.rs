use super::*;
use crate::diff::Diff;
use crate::effect::Effect;
use crate::project::ClipId;
use crate::test_util::{clip, r};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
struct Items {
    /// Stored numbers.
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
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Test>()?;
        state.items.push(self.0);
        project.set_state::<Test>(&state)?;
        Ok(Diff::note(format!("+{}", self.0)))
    }
}

#[derive(Debug)]
struct Fail;
impl Command for Fail {
    fn name(&self) -> &'static str {
        "test fail"
    }
    fn apply(&self, _: &mut Project) -> Result<Diff, CommandError> {
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
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let id = clip(project, 1.0);
        project.clip_mut(id).unwrap().trim = r(0.0, 5.0);
        Ok(Diff::added(id))
    }
}

fn items(p: &Project) -> Vec<u32> {
    p.state::<Test>().unwrap().items
}

#[test]
fn undo_and_redo_restore_exact_states() {
    let mut p = Project::default();
    let mut h = History::default();
    assert_eq!(h.apply(&mut p, &Push(1)).unwrap().to_string(), "+1");
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
fn a_change_made_elsewhere_is_pushed_as_if_applied_here() {
    let (mut here, mut elsewhere) = (Project::default(), Project::default());
    let (mut mine, mut theirs) = (History::default(), History::default());
    mine.apply(&mut here, &Push(1)).unwrap();
    theirs.apply(&mut elsewhere, &Push(1)).unwrap();
    mine.apply(&mut here, &Push(7)).unwrap();
    mine.undo(&mut here);
    theirs.apply(&mut elsewhere, &Push(2)).unwrap();
    let change = theirs.changes().last().unwrap().clone();
    mine.push(&mut here, change);
    assert_eq!(here, elsewhere);
    assert!(!mine.can_redo(), "a new step clears redo");
    mine.undo(&mut here);
    assert_eq!(items(&here), vec![1]);
}

#[test]
fn batch_is_one_step_and_all_or_nothing() {
    let mut p = Project::default();
    let mut h = History::default();
    assert_eq!(
        h.apply_batch(&mut p, "apply", &[&Push(1), &Push(2)])
            .unwrap()
            .to_string(),
        "+1; +2"
    );
    assert_eq!(h.changes().len(), 1);
    let error = h
        .apply_batch(&mut p, "apply", &[&Push(3), &Fail])
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "шаг 2 (test fail): нельзя. Ни один шаг не применён"
    );
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
        fn apply(&self, _: &mut Project) -> Result<Diff, CommandError> {
            Ok(Diff::note("без изменений"))
        }
    }
    let mut p = Project::default();
    let mut h = History::default();
    h.apply(&mut p, &Nothing).unwrap();
    assert!(!h.can_undo());
}

#[test]
fn change_without_diff_is_rolled_back() {
    #[derive(Debug)]
    struct Silent;
    impl Command for Silent {
        fn name(&self) -> &'static str {
            "silent"
        }
        fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
            project
                .effects
                .insert("test".to_owned(), serde_json::json!({"items": [1]}));
            Ok(Diff::default())
        }
    }
    let mut p = Project::default();
    let mut h = History::default();
    let error = h.apply(&mut p, &Silent).unwrap_err().to_string();
    assert!(error.contains("не описала изменение"), "{error}");
    assert!(p.effects.is_empty() && !h.can_undo());
}

#[test]
fn not_found_lists_existing_ids_and_how_to_see_them() {
    let id = |n| ClipId::from_number(n);
    let error = CommandError::not_found(id(99), (1..=14).map(id)).to_string();
    assert_eq!(
        error,
        "c99 не найден (есть c1..c14); список: devcast ls clips"
    );
    let error = CommandError::not_found(id(2), []).to_string();
    assert_eq!(
        error,
        "c2 не найден (нет ни одного клипа); список: devcast ls clips"
    );
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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
struct Counted {
    /// Ids of the stored objects.
    items: Vec<ClipId>,
    /// Next free id.
    ids: IdCounter<ClipKind>,
}

struct CountedEffect;
impl Effect for CountedEffect {
    const ID: &'static str = "counted";
    const TITLE: &'static str = "Тест";
    type State = Counted;
}

#[derive(Debug)]
struct AddCounted;
impl Command for AddCounted {
    fn name(&self) -> &'static str {
        "counted add"
    }
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CountedEffect>()?;
        let id = state.ids.allocate();
        state.items.push(id);
        project.set_state::<CountedEffect>(&state)?;
        Ok(Diff::added(id))
    }
}

#[derive(Debug)]
struct AddClip;
impl Command for AddClip {
    fn name(&self) -> &'static str {
        "clip add"
    }
    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        Ok(Diff::added(clip(project, 1.0)))
    }
}

#[test]
fn undo_and_redo_never_lower_id_counters() {
    let mut p = Project::default();
    let mut h = History::default();
    h.apply(&mut p, &AddCounted).unwrap();
    h.apply(&mut p, &AddCounted).unwrap();
    h.undo(&mut p);
    h.undo(&mut p);
    h.apply(&mut p, &AddCounted).unwrap();
    let items = p.state::<CountedEffect>().unwrap().items;
    assert_eq!(items, [ClipId::from_number(3)]);
    h.undo(&mut p);
    assert_eq!(p.state::<CountedEffect>().unwrap().ids.next_number(), 4);
    h.apply(&mut p, &AddClip).unwrap();
    h.undo(&mut p);
    h.apply(&mut p, &AddClip).unwrap();
    assert_eq!(p.clips[0].id, ClipId::from_number(2));
    h.undo(&mut p);
    h.redo(&mut p);
    assert_eq!(p.clip_ids.next_number(), 3);
}

#[test]
fn steps_of_one_group_are_one_undo_step() {
    let mut p = Project::default();
    let mut h = History::default();
    h.apply(&mut p, &Push(1)).unwrap();
    h.group(Some("ai-1".to_owned()));
    h.apply(&mut p, &Push(2)).unwrap();
    h.apply(&mut p, &Push(3)).unwrap();
    h.group(Some("ai-2".to_owned()));
    h.apply(&mut p, &Push(4)).unwrap();
    h.group(None);
    h.apply(&mut p, &Push(5)).unwrap();
    h.apply(&mut p, &Push(6)).unwrap();
    assert_eq!(h.changes().len(), 5, "1, the group 2+3, the group 4, 5, 6");
    assert_eq!(h.changes()[1].summary, "+2; +3");
    for left in [
        vec![1, 2, 3, 4, 5],
        vec![1, 2, 3, 4],
        vec![1, 2, 3],
        vec![1],
    ] {
        h.undo(&mut p);
        assert_eq!(items(&p), left);
    }
    h.redo(&mut p);
    assert_eq!(items(&p), vec![1, 2, 3], "the group comes back whole");
}

#[test]
fn a_pushed_step_joins_its_group_even_when_it_already_holds_it() {
    let (mut here, mut there) = (Project::default(), Project::default());
    let (mut mine, mut theirs) = (History::default(), History::default());
    theirs.group(Some("ai".to_owned()));
    theirs.apply(&mut there, &Push(1)).unwrap();
    mine.push(&mut here, theirs.changes()[0].clone());
    theirs.apply(&mut there, &Push(2)).unwrap();
    // Another writer sends its top step, which already folds the first one in.
    mine.push(&mut here, theirs.changes()[0].clone());
    assert_eq!(items(&here), vec![1, 2]);
    assert_eq!(mine.changes().len(), 1);
    mine.undo(&mut here);
    assert!(here.effects.is_empty());
}
