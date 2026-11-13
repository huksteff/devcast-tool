//! `beautify` with a pretend agent, `recipes` in a settings folder of the test, and the undo
//! step of one agent run (`DEVCAST_GROUP`).

use std::path::Path;

use dc_agent::Found;
use dc_settings::Agent;

use crate::run::{Local, Outcome, run_with};
use crate::test_util::{EXPORTS, Fixture, NO_APP, NO_RECORDING, defaults_file};

fn run(f: &Fixture, args: &[&str], agent: Option<&Found>, group: Option<&str>) -> Outcome {
    let defaults = defaults_file(f._tmp.path());
    let local = Local {
        settings: defaults.parent(),
        app_channel: Some(NO_APP),
        record_channel: Some(NO_RECORDING),
        export_channel: Some(EXPORTS),
        agent,
        group,
        ..Local::default()
    };
    let args = args.iter().map(|&a| a.to_owned()).collect();
    run_with(args, &f.cwd, &f.registry, local)
}

fn ok(outcome: Outcome) -> String {
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    outcome.stdout
}

#[test]
fn the_edits_of_one_agent_run_are_one_undo_step() {
    let f = Fixture::new();
    ok(run(&f, &["test", "add", "--secs", "2"], None, None));
    ok(run(&f, &["test", "add", "--secs", "2"], None, Some("ai-1")));
    ok(run(&f, &["test", "add", "--secs", "2"], None, Some("ai-1")));
    assert_eq!(f.clips(), 3);
    ok(run(&f, &["undo"], None, None));
    assert_eq!(f.clips(), 1, "both edits of the run go back together");
    ok(run(&f, &["redo"], None, None));
    assert_eq!(f.clips(), 3);
}

#[test]
fn agent_cli_edits_reach_the_editor_and_each_run_has_its_own_undo_step() {
    let f = Fixture::new();
    let editor = crate::fake_editor::FakeEditor::start(&f, "agent-cli");
    let defaults = defaults_file(f._tmp.path());
    for (group, secs) in [("ai-1", "2"), ("ai-1", "3"), ("ai-2", "4")] {
        let local = Local {
            settings: defaults.parent(),
            app_channel: Some(&editor.channel),
            record_channel: Some(NO_RECORDING),
            export_channel: Some(EXPORTS),
            group: Some(group),
            ..Local::default()
        };
        let args = ["test", "add", "--secs", secs].map(str::to_owned).to_vec();
        ok(run_with(args, &f.cwd, &f.registry, local));
    }
    let (project, mut history) = editor.stop();
    assert_eq!(project.clips.len(), 3);
    let mut restored = project.clone();
    assert!(history.undo(&mut restored).is_some());
    assert_eq!(restored.clips.len(), 2, "resume has a separate group");
    assert!(history.undo(&mut restored).is_some());
    assert!(
        restored.clips.is_empty(),
        "first run's two commands undo together"
    );
    assert!(!history.can_undo());
    assert!(history.redo(&mut restored).is_some());
    assert!(history.redo(&mut restored).is_some());
    assert_eq!(restored, project);
}

#[test]
fn the_agent_may_not_undo_nor_touch_the_project_as_a_whole() {
    let f = Fixture::new();
    ok(run(&f, &["test", "add", "--secs", "2"], None, None));
    for line in [
        &["undo"][..],
        &["redo"],
        &["save"],
        &["discard"],
        &["config", "show"],
    ] {
        let out = run(&f, line, None, Some("ai-1"));
        assert_eq!(out.code, 2, "{line:?}");
        assert!(out.stderr.contains("«Сделать красиво»"), "{}", out.stderr);
    }
    assert_eq!(f.clips(), 1, "the user's step is still there");
}

#[test]
fn the_agent_writes_files_and_edits_only_inside_its_project() {
    let f = Fixture::new();
    ok(run(&f, &["test", "add", "--secs", "2"], None, None));
    // The folder of this crate is neither the project nor the temp folder.
    let outside = Path::new(env!("CARGO_MANIFEST_DIR")).join("agent-wrote-this.txt");
    let outside = outside.to_string_lossy().into_owned();
    let other = f._tmp.path().join("other.devcast");
    std::fs::create_dir_all(&other).unwrap();
    let other = other.to_string_lossy().into_owned();
    for line in [
        &["captions", "export", "-o", &outside][..],
        &["frame", "--at", "0", "-o", &outside],
        &["sheet", "-o", &outside],
        &["--project", &other, "ls", "clips"],
        &["style", "copy", "--from", &other, "--to", &other],
    ] {
        let out = run(&f, line, None, Some("ai-1"));
        assert_eq!(out.code, 2, "{line:?}: {}", out.stderr);
        assert!(out.stderr.contains("«Сделать красиво»"), "{}", out.stderr);
    }
    assert!(!Path::new(&outside).exists());
    // Inside the project the check lets the command through (there are no captions to write).
    let inside = f.cwd.join("субтитры.txt").to_string_lossy().into_owned();
    let allowed = run(
        &f,
        &["captions", "export", "-o", &inside],
        None,
        Some("ai-1"),
    );
    assert!(
        allowed.stderr.contains("captions gen"),
        "{}",
        allowed.stderr
    );
    let own = f.dir.root().to_string_lossy().into_owned();
    ok(run(
        &f,
        &["--project", &own, "ls", "clips"],
        None,
        Some("ai-1"),
    ));
    let user = run(&f, &["captions", "export", "-o", &outside], None, None);
    assert!(
        user.stderr.contains("captions gen"),
        "the user writes where they like"
    );
}

#[test]
fn a_wrong_command_line_names_the_argument() {
    let f = Fixture::new();
    let out = run(&f, &["ls", "clips", "--nope"], None, None);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--nope"), "{}", out.stderr);
    assert!(!out.stderr.contains("Usage:"), "{}", out.stderr);
}

#[test]
fn own_recipes_are_added_checked_and_listed() {
    let f = Fixture::new();
    let added = ok(run(
        &f,
        &["recipes", "add", "Лого", "Вставь лого в угол"],
        None,
        None,
    ));
    assert!(added.contains("рецепт «Лого» добавлен"), "{added}");
    ok(run(&f, &["recipes", "check", "zooms", "--off"], None, None));
    ok(run(&f, &["recipes", "model", "sonnet"], None, None));
    let ls = ok(run(&f, &["recipes", "ls"], None, None));
    assert!(ls.contains("[x] Лого — Лого (свой)") && ls.contains("[ ] zooms — Зумы по кликам"));
    assert!(ls.contains("модель: sonnet"), "{ls}");
    let taken = run(&f, &["recipes", "add", "Лого", "ещё"], None, None);
    assert_eq!(taken.code, 1);
    assert!(taken.stderr.contains("уже есть"), "{}", taken.stderr);
    let unknown = run(&f, &["recipes", "check", "нет"], None, None);
    assert_eq!(unknown.code, 2);
}

#[test]
fn a_dry_run_shows_the_request_without_starting_the_agent() {
    let f = Fixture::new();
    let agent = Found {
        agent: Agent::Claude,
        path: f.cwd.join("нет такой программы.exe"),
    };
    let args = [
        "beautify",
        "--recipe",
        "slips",
        "--ask",
        "не трогай 0:42",
        "--dry-run",
    ];
    let shown = ok(run(&f, &args, Some(&agent), None));
    assert!(shown.contains("devcast transcript") && shown.contains("не трогай 0:42"));
    assert!(!shown.contains("Убери паузы"), "only the recipe named");
    let nothing = run(&f, &["beautify", "--recipe", "нет"], Some(&agent), None);
    assert_eq!(nothing.code, 2);
}

/// A pretend Claude Code: a batch file that ignores its arguments and writes `answer`.
#[cfg(windows)]
fn pretend(dir: &Path, answer: &str) -> Found {
    let lines = dir.join("answer.jsonl");
    std::fs::write(&lines, answer).unwrap();
    let script = dir.join("agent.cmd");
    std::fs::write(&script, format!("@type \"{}\"\r\n", lines.display())).unwrap();
    Found {
        agent: Agent::Claude,
        path: script,
    }
}

#[cfg(windows)]
#[test]
fn a_run_answers_with_the_agents_last_words_and_its_session() {
    let f = Fixture::new();
    let answer = concat!(
        r#"{"type":"system","subtype":"init","session_id":"s-9"}"#,
        "\n",
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"devcast ls"}}]}}"#,
        "\n",
        r#"{"type":"result","is_error":false,"result":"Убрал паузы.","session_id":"s-9"}"#,
        "\n"
    );
    let agent = pretend(f._tmp.path(), answer);
    let said = ok(run(
        &f,
        &["beautify", "--ask", "убери паузы"],
        Some(&agent),
        None,
    ));
    assert!(said.starts_with("Убрал паузы.\nкоманд: 1"), "{said}");
    assert!(said.contains("devcast beautify --resume s-9"), "{said}");
    let failing = r#"{"type":"result","is_error":true,"result":"Not logged in"}"#;
    let agent = pretend(f._tmp.path(), failing);
    let failed = run(&f, &["beautify", "--ask", "x"], Some(&agent), None);
    assert_eq!(failed.code, 1);
    assert!(
        failed.stderr.contains("Claude Code: Not logged in"),
        "{}",
        failed.stderr
    );
}

#[cfg(windows)]
#[test]
fn codex_beautify_returns_its_session_and_supports_a_followup() {
    let f = Fixture::new();
    let script = f._tmp.path().join("mock.ps1");
    std::fs::write(&script, include_str!("../../../fixtures/agents/codex.ps1")).unwrap();
    let shim = f._tmp.path().join("codex.cmd");
    std::fs::write(&shim, include_str!("../../../fixtures/agents/codex.cmd")).unwrap();
    let agent = Found {
        agent: Agent::Codex,
        path: shim,
    };
    for args in [
        vec![
            "beautify",
            "--agent",
            "codex",
            "--ask",
            "добавь зумы",
            "--json",
        ],
        vec![
            "beautify",
            "--agent",
            "codex",
            "--resume",
            "codex-9",
            "--ask",
            "доделай",
            "--json",
        ],
    ] {
        let out = ok(run(&f, &args, Some(&agent), None));
        let json: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(json["session"], "codex-9");
        assert_eq!(json["said"], "Добавил зумы.");
        assert_eq!(json["commands"], 1);
    }
}
