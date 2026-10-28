use crate::tests::{Fixture, done};

fn plan(f: &Fixture, name: &str, text: &str) -> String {
    let path = f.cwd.join(name);
    std::fs::write(&path, text).unwrap();
    name.to_owned()
}

#[test]
fn plan_is_one_step_and_undo_redo_restore_it() {
    let f = Fixture::new();
    let file = plan(
        &f,
        "план.json",
        r#"[{"op":"clip add","secs":2},{"op":"clip add","secs":1},{"op":"clip rm","id":"c1"}]"#,
    );
    assert_eq!(
        f.run(&format!("apply {file} --dry-run")),
        done(
            "dry-run, проект не изменён: клипы +2 c1,c2; клипы -1 c1; длительность 0:00.000→0:01.000"
        )
    );
    assert_eq!(f.clips(), 0);
    assert_eq!(
        f.run(&format!("apply {file}")),
        done("клипы +2 c1,c2; клипы -1 c1; длительность 0:00.000→0:01.000")
    );
    let saved = f.dir.load().unwrap();
    assert_eq!(f.run("clip add --secs 3").code, 0);
    assert_eq!(
        f.run("history"),
        done(
            "отменить (devcast undo N):\n  1  clip add  клипы +1 c3\n  2  apply     клипы +2 c1,c2; клипы -1 c1"
        )
    );
    assert_eq!(
        f.run("undo 2"),
        done("отменено: клипы +1 c3; клипы +2 c1,c2; клипы -1 c1; длительность 0:04.000→0:00.000")
    );
    assert_eq!(f.clips(), 0);
    assert_eq!(
        f.run("redo"),
        done("возвращено: клипы +2 c1,c2; клипы -1 c1; длительность 0:00.000→0:01.000")
    );
    assert_eq!(f.dir.load().unwrap(), saved, "redo gives the same project");
    assert_eq!(
        f.run("history"),
        done(
            "отменить (devcast undo N):\n  1  apply     клипы +2 c1,c2; клипы -1 c1\nвернуть (devcast redo):\n  1  clip add  клипы +1 c3"
        )
    );
}

#[test]
fn history_survives_between_runs_and_limits_are_explained() {
    let f = Fixture::new();
    assert_eq!(f.run("history"), done("история пуста"));
    let out = f.run("undo");
    assert_eq!(
        (out.code, out.stderr.as_str()),
        (1, "нечего отменять: таких шагов в истории нет\n")
    );
    f.run("clip add --secs 1");
    let out = f.run("undo 3");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("доступно шагов — 1"), "{}", out.stderr);
    assert_eq!(f.run("undo 0").code, 2);
    assert_eq!(f.run("undo many").code, 2);
    let out = f.run("undo --dry-run");
    assert!(
        out.stdout
            .starts_with("dry-run, проект не изменён: отменено: клипы +1 c1")
    );
    assert_eq!(f.clips(), 1);
    let json: serde_json::Value = serde_json::from_str(&f.run("history --json").stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"undo": [{"name": "clip add", "summary": "клипы +1 c1"}], "redo": []})
    );
}

#[test]
fn bad_plans_change_nothing() {
    let f = Fixture::new();
    let file = plan(
        &f,
        "bad.json",
        r#"[{"op":"clip add","secs":1},{"op":"clip rm","id":"c7"}]"#,
    );
    let out = f.run(&format!("apply {file}"));
    assert_eq!(out.code, 1);
    assert!(out.stderr.starts_with("шаг 2"), "{}", out.stderr);
    assert_eq!(f.clips(), 0);
    let file = plan(&f, "typo.json", r#"[{"op":"clip ad","secs":1}]"#);
    assert_eq!(f.run(&format!("apply {file}")).code, 2);
    let out = f.run("apply нет.json");
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("не удалось прочитать план"),
        "{}",
        out.stderr
    );
}

#[test]
fn save_commits_unsaved_editor_state() {
    let f = Fixture::new();
    assert_eq!(
        f.run("save"),
        done("нечего сохранять: несохранённых изменений нет")
    );
    f.dir.save_session(&dc_core::Project::default()).unwrap();
    f.run("clip add --secs 1");
    assert_eq!(f.clips(), 0);
    assert_eq!(
        f.run("save --dry-run"),
        done("dry-run, проект не изменён: рабочее состояние сохранится в project.json")
    );
    assert!(f.dir.has_unsaved_changes());
    assert_eq!(f.run("save"), done("сохранено в project.json"));
    assert_eq!(f.clips(), 1);
    assert!(!f.dir.has_unsaved_changes());
    assert_eq!(
        f.run("undo"),
        done("отменено: клипы +1 c1; длительность 0:01.000→0:00.000")
    );
}
