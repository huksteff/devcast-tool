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
        "captions",
        "--ask",
        "не трогай 0:42",
        "--dry-run",
    ];
    let shown = ok(run(&f, &args, Some(&agent), None));
    assert!(shown.contains("devcast captions gen") && shown.contains("не трогай 0:42"));
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
