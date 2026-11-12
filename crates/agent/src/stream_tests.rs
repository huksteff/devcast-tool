use super::*;

/// A Claude Code run in the shape `claude -p --output-format stream-json --verbose` writes it
/// (2.1.309): init, a call of `Bash`, its result, the words, the result line.
pub const CLAUDE_RUN: &str = r#"{"type":"system","subtype":"init","cwd":"C:\\Урок.devcast","session_id":"5a1e","tools":["Bash"],"model":"claude-sonnet"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Смотрю паузы."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"devcast silences","description":"Паузы"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"0:09.250–0:11.150","is_error":false}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"Bash","input":{"command":"devcast cut 9.4-11"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t2","content":"нельзя","is_error":true}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Вырезал одну паузу."}]}}
{"type":"result","subtype":"success","is_error":false,"result":"Вырезал одну паузу.","session_id":"5a1e"}
"#;

fn read(lines: &str) -> Vec<Event> {
    lines.lines().flat_map(line).collect()
}

fn s(text: &str) -> String {
    text.to_owned()
}

#[test]
fn a_claude_run_is_its_commands_words_and_end() {
    let events = read(CLAUDE_RUN);
    let expected = vec![
        Event::Session(s("5a1e")),
        Event::Said(s("Смотрю паузы.")),
        Event::Command(s("devcast silences")),
        Event::Done { ok: true },
        Event::Command(s("devcast cut 9.4-11")),
        Event::Done { ok: false },
        Event::Said(s("Вырезал одну паузу.")),
        Event::Finished(Ok(s("Вырезал одну паузу."))),
    ];
    assert_eq!(events, expected);
}

#[test]
fn a_claude_error_ends_the_run_with_its_words() {
    // As a Claude Code that is not signed in answers.
    let line = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login","session_id":"x"}"#;
    let events = read(line);
    assert_eq!(
        events,
        vec![Event::Finished(Err(s("Not logged in · Please run /login")))]
    );
    assert!(read("не JSON\n{\"type\":\"rate_limit\"}").is_empty());
}

#[test]
fn claude_code_tells_how_full_the_limits_are() {
    // As Claude Code 2.1.309 writes it (2026-11-04).
    let line = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.19,"status":"allowed"},"seven_day":{"utilization":0.35,"status":"allowed"}}}}"#;
    let limits = crate::usage::Limits {
        five_hour: Some(0.19),
        seven_day: Some(0.35),
    };
    assert_eq!(read(line), vec![Event::Limits(limits)]);
}

#[test]
fn a_run_that_never_started_tells_why() {
    // As Claude Code 2.1.321 prints an expired sign-in.
    let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"num_turns":0,"errors":["Your Claude session has expired. Sign in again to continue (claude auth login)."]}"#;
    let events = super::line(line);
    let Some(Event::Finished(Err(why))) = events.last() else {
        panic!("{events:?}");
    };
    assert!(why.contains("claude auth login"), "{why}");
}
