use std::time::{Duration, Instant};

use super::*;
use crate::stream::tests::CLAUDE_RUN;

fn found(agent: Agent) -> Found {
    Found {
        agent,
        path: PathBuf::from(r"C:\bin\agent.exe"),
    }
}

fn ask<'a>(found: &'a Found, resume: Option<&'a str>) -> Ask<'a> {
    Ask {
        found,
        model: Some("sonnet"),
        project: Path::new(r"C:\Видео\Урок 1.devcast"),
        devcast: Path::new(r"C:\Program Files\DevCast"),
        system: Path::new(r"C:\Temp\system.md"),
        request: "Убери паузы.",
        resume,
        group: "ai-7",
    }
}

fn strings(launch: &Launch) -> Vec<String> {
    launch
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn has(args: &[String], pair: [&str; 2]) -> bool {
    args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1])
}

#[test]
fn claude_code_may_only_run_devcast_and_gets_the_skill_as_a_file() {
    let claude = found(Agent::Claude);
    let launch = launch(&ask(&claude, Some("5a1e")));
    let args = strings(&launch);
    assert!(has(&args, ["--tools", TOOLS]) && has(&args, ["--allowedTools", ALLOWED[0]]));
    assert!(
        has(&args, [ALLOWED[0], ALLOWED[1]]),
        "PowerShell may run devcast too"
    );
    assert!(
        has(&args, ["--permission-prompts", "none"]),
        "nothing waits for a person"
    );
    assert!(has(
        &args,
        ["--append-system-prompt-file", r"C:\Temp\system.md"]
    ));
    assert!(has(&args, ["--model", "sonnet"]) && has(&args, ["--resume", "5a1e"]));
    assert_eq!(launch.input, "Убери паузы.", "the request goes to stdin");
    assert_eq!(launch.cwd, Path::new(r"C:\Видео\Урок 1.devcast"));
    let env = |name: &str| {
        launch
            .env
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(env(GROUP_ENV), Some("ai-7".into()));
    let path = env("PATH").unwrap();
    let first = std::env::split_paths(&path).next().unwrap();
    assert_eq!(
        first,
        Path::new(r"C:\Program Files\DevCast"),
        "our devcast first"
    );
}

#[test]
fn codex_writes_only_the_project_and_carries_the_skill_in_its_first_request() {
    let codex = found(Agent::Codex);
    let first = launch(&ask(&codex, None));
    let args = strings(&first);
    assert_eq!(args[0], "exec");
    assert!(has(&args, ["--sandbox", "workspace-write"]));
    assert_eq!(
        args.last().map(String::as_str),
        Some("-"),
        "the request from stdin"
    );
    assert!(first.input.starts_with(crate::prompt::BRIEF) && first.input.ends_with("Убери паузы."));
    let more = launch(&ask(&codex, Some("th-1")));
    assert!(has(&strings(&more), ["resume", "th-1"]));
    assert_eq!(more.input, "Убери паузы.");
}

/// A pretend agent: `cmd /C type <file>` writes the lines of `answer`.
#[cfg(windows)]
fn pretend(dir: &Path, answer: &str) -> Launch {
    let file = dir.join("ответ.jsonl");
    std::fs::write(&file, answer).unwrap();
    Launch {
        agent: Agent::Claude,
        program: PathBuf::from("cmd"),
        args: vec!["/C".into(), "type".into(), file.into()],
        cwd: dir.to_path_buf(),
        env: Vec::new(),
        input: String::new(),
    }
}

#[cfg(windows)]
#[test]
fn a_run_hands_its_events_in_order_and_ends() {
    let tmp = tempfile::tempdir().unwrap();
    let job = Job::start(pretend(tmp.path(), CLAUDE_RUN), || {}).unwrap();
    let events: Vec<Event> = std::iter::from_fn(|| job.next()).collect();
    assert_eq!(events.first(), Some(&Event::Session("5a1e".to_owned())));
    assert_eq!(
        events.last(),
        Some(&Event::Finished(Ok("Вырезал одну паузу.".to_owned())))
    );
}

#[cfg(windows)]
#[test]
fn a_run_that_ends_without_an_answer_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let job = Job::start(pretend(tmp.path(), "просто текст\n"), || {}).unwrap();
    let events: Vec<Event> = std::iter::from_fn(|| job.next()).collect();
    assert!(matches!(events.as_slice(), [Event::Finished(Err(_))]));
}

#[cfg(windows)]
#[test]
fn a_run_that_fails_tells_what_the_agent_said() {
    let tmp = tempfile::tempdir().unwrap();
    let old = Launch {
        program: PathBuf::from("cmd"),
        args: vec!["/C".into(), "echo error: unknown option '--x' 1>&2".into()],
        ..pretend(tmp.path(), "")
    };
    let job = Job::start(old, || {}).unwrap();
    let events: Vec<Event> = std::iter::from_fn(|| job.next()).collect();
    let [Event::Finished(Err(why))] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert!(
        why.contains("unknown option '--x'") && why.contains("claude update"),
        "{why}"
    );
    assert!(no_answer("").ends_with("попробуй ещё раз"));
    assert!(no_answer("Not logged in").contains("(Not logged in)"));
}

#[cfg(windows)]
#[test]
fn stop_ends_a_long_run() {
    let tmp = tempfile::tempdir().unwrap();
    let long = Launch {
        program: PathBuf::from("cmd"),
        args: vec!["/C".into(), "ping -n 600 127.0.0.1 >nul".into()],
        ..pretend(tmp.path(), "")
    };
    let mut job = Job::start(long, || {}).unwrap();
    let started = Instant::now();
    job.stop();
    assert!(started.elapsed() < Duration::from_secs(30));
    let events: Vec<Event> = std::iter::from_fn(|| job.next()).collect();
    assert!(
        matches!(events.as_slice(), [Event::Finished(Err(_))]),
        "{events:?}"
    );
}
