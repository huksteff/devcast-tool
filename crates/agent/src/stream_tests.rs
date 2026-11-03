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

fn read(agent: Agent, lines: &str) -> Vec<Event> {
    let mut reader = Reader::new(agent);
    lines.lines().flat_map(|line| reader.line(line)).collect()
}

fn s(text: &str) -> String {
    text.to_owned()
}

#[test]
fn a_claude_run_is_its_commands_words_and_end() {
    let events = read(Agent::Claude, CLAUDE_RUN);
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
    let events = read(Agent::Claude, line);
    assert_eq!(
        events,
        vec![Event::Finished(Err(s("Not logged in · Please run /login")))]
    );
    assert!(read(Agent::Claude, "не JSON\n{\"type\":\"rate_limit\"}").is_empty());
}

#[test]
fn a_codex_run_is_its_commands_words_and_end() {
    let lines = r#"{"type":"thread.started","thread_id":"th-1"}
{"type":"turn.started"}
{"type":"item.started","item":{"id":"i1","type":"command_execution","command":"devcast silences","status":"in_progress"}}
{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"devcast silences","exit_code":0,"status":"completed"}}
{"type":"item.started","item":{"id":"i2","type":"command_execution","command":"devcast cut 1-2"}}
{"type":"item.completed","item":{"id":"i2","type":"command_execution","exit_code":1,"status":"failed"}}
{"type":"item.completed","item":{"id":"i3","type":"agent_message","text":"Готово."}}
{"type":"turn.completed","usage":{"input_tokens":10}}
"#;
    let expected = vec![
        Event::Session(s("th-1")),
        Event::Command(s("devcast silences")),
        Event::Done { ok: true },
        Event::Command(s("devcast cut 1-2")),
        Event::Done { ok: false },
        Event::Said(s("Готово.")),
        Event::Finished(Ok(s("Готово."))),
    ];
    assert_eq!(read(Agent::Codex, lines), expected);
    let failed = r#"{"type":"turn.failed","error":{"message":"квота"}}"#;
    assert_eq!(
        read(Agent::Codex, failed),
        vec![Event::Finished(Err(s("квота")))]
    );
}
