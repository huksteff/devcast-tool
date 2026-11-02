use dc_core::{Command, CommandError, Diff};

use super::*;

#[derive(Debug)]
struct Set(u32);
impl Command for Set {
    fn name(&self) -> &'static str {
        "test set"
    }
    fn apply(&self, project: &mut Project) -> std::result::Result<Diff, CommandError> {
        project.effects.insert("test".to_owned(), self.0.into());
        Ok(Diff::note(format!("test={}", self.0)))
    }
}

struct Fixture {
    _tmp: tempfile::TempDir,
    log: HistoryLog,
    project: Project,
    history: History,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("История проекта.devcast")).unwrap();
        let log = HistoryLog::new(&dir);
        Self {
            _tmp: tmp,
            log,
            project: Project::default(),
            history: History::default(),
        }
    }

    fn set(&mut self, value: u32) {
        self.history.apply(&mut self.project, &Set(value)).unwrap();
        let change = self.history.changes().last().unwrap().clone();
        self.log.append(&[Event::Do(change)]).unwrap();
    }

    fn undo(&mut self) {
        self.history.undo(&mut self.project).unwrap();
        self.log.append(&[Event::Undo]).unwrap();
    }

    fn reload(&self) -> Loaded {
        self.log.load(&self.project, 1000).unwrap()
    }

    fn value(project: &Project) -> Option<&serde_json::Value> {
        project.effects.get("test")
    }
}

#[test]
fn missing_file_is_an_empty_history() {
    let f = Fixture::new();
    let loaded = f.reload();
    assert!(!loaded.history.can_undo() && loaded.warnings.is_empty() && !loaded.needs_rewrite);
}

#[test]
fn stacks_survive_a_restart() {
    let mut f = Fixture::new();
    f.set(1);
    f.set(2);
    f.set(3);
    f.undo();
    let mut loaded = f.reload();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.history.changes(), f.history.changes());
    assert_eq!(loaded.history.redo_changes(), f.history.redo_changes());
    let mut project = f.project.clone();
    loaded.history.redo(&mut project).unwrap();
    assert_eq!(Fixture::value(&project), Some(&3.into()));
}

#[test]
fn torn_last_line_is_skipped() {
    let mut f = Fixture::new();
    f.set(1);
    f.set(2);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&f.log.path)
        .unwrap();
    file.write_all(br#"{"do":{"name":"test set","summ"#)
        .unwrap();
    let loaded = f.reload();
    assert_eq!(loaded.history.changes().len(), 2);
    assert!(loaded.warnings[0].contains("оборвана") && loaded.needs_rewrite);
}

#[test]
fn corrupt_middle_line_drops_older_steps_only() {
    let mut f = Fixture::new();
    f.set(1);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&f.log.path)
        .unwrap()
        .write_all(b"garbage\n")
        .unwrap();
    f.set(2);
    f.set(3);
    let mut loaded = f.reload();
    assert!(loaded.warnings[0].contains("№2"), "{:?}", loaded.warnings);
    assert_eq!(loaded.history.changes().len(), 2);
    let mut project = f.project.clone();
    loaded.history.undo(&mut project).unwrap();
    loaded.history.undo(&mut project).unwrap();
    assert_eq!(
        Fixture::value(&project),
        Some(&1.into()),
        "back to the state before the kept steps"
    );
    assert!(!loaded.history.can_undo());
}

#[test]
fn history_that_does_not_fit_the_project_resets() {
    let mut f = Fixture::new();
    f.set(1);
    f.set(2);
    let on_disk = {
        let mut p = f.project.clone();
        p.effects.insert("test".to_owned(), 1.into());
        p
    };
    let loaded = f.log.load(&on_disk, 1000).unwrap();
    assert!(!loaded.history.can_undo() && !loaded.history.can_redo());
    assert!(loaded.warnings[0].contains("начата заново"));
}

#[test]
fn rewrite_compacts_to_the_current_stacks() {
    let mut f = Fixture::new();
    for value in 1..=40 {
        f.set(value);
        f.undo();
    }
    f.set(100);
    f.set(101);
    f.undo();
    assert!(f.reload().needs_rewrite);
    f.log.rewrite(&f.history).unwrap();
    let text = std::fs::read_to_string(&f.log.path).unwrap();
    assert_eq!(text.lines().count(), 4, "version, do 100, do 101, undo");
    let loaded = f.reload();
    assert!(!loaded.needs_rewrite && loaded.warnings.is_empty());
    assert_eq!(loaded.history.changes(), f.history.changes());
    assert_eq!(loaded.history.redo_changes(), f.history.redo_changes());
}

#[test]
fn the_log_starts_with_its_schema_version() {
    let mut f = Fixture::new();
    f.set(1);
    let text = std::fs::read_to_string(&f.log.path).unwrap();
    let first = text.lines().next().unwrap();
    assert_eq!(first, format!(r#"{{"schema_version":{SCHEMA_VERSION}}}"#));
    assert_eq!(f.reload().history.changes().len(), 1);
}

#[test]
fn a_log_of_another_schema_version_starts_anew() {
    let mut f = Fixture::new();
    f.set(1);
    f.set(2);
    let text = std::fs::read_to_string(&f.log.path).unwrap();
    let (_, events) = text.split_once('\n').unwrap();
    let other = format!("{{\"schema_version\":{}}}\n{events}", SCHEMA_VERSION + 1);
    let newer = format!("схемой проекта версии {}", SCHEMA_VERSION + 1);
    for (log, found) in [(other, newer), (events.to_owned(), "без версии".to_owned())] {
        std::fs::write(&f.log.path, log).unwrap();
        let loaded = f.reload();
        assert!(!loaded.history.can_undo() && loaded.needs_rewrite);
        assert!(loaded.warnings[0].contains(&found), "{:?}", loaded.warnings);
    }
}
