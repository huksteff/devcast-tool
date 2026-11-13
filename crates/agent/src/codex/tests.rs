use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{DISABLED, Session, policy, state::State};
use crate::{Ask, Event, Found, Job};

fn session(group: &str, resume: Option<&str>) -> Session {
    Session {
        exe: PathBuf::from("C:/Program Files/DevCast/devcast.exe"),
        project: PathBuf::from("C:/Видео/Урок 1.devcast"),
        group: group.into(),
        model: Some("chosen-model".into()),
        resume: resume.map(str::to_owned),
    }
}

fn config() -> Value {
    let features: serde_json::Map<String, Value> = DISABLED
        .iter()
        .map(|flag| ((*flag).into(), json!(false)))
        .collect();
    json!({"features":features, "web_search":"disabled", "mcp_servers":{
        "foreign.server":{"command":"foreign.exe", "enabled":true}
    }})
}

fn ready(group: &str) -> State {
    let mut state = State::new(session(group, None), "сделай монтаж".into());
    for message in handshake() {
        state.handle(&message).unwrap();
    }
    state
}

fn handshake() -> Vec<Value> {
    vec![
        json!({"id":1, "result":{}}),
        json!({"id":2, "result":{"config":config()}}),
        json!({"id":3, "result":{"thread":{"id":"th-1"}, "model":"chosen-model",
            "approvalPolicy":"never", "sandbox":{"type":"readOnly"}}}),
        json!({"id":4, "result":{"data":[], "nextCursor":null}}),
        json!({"id":5, "result":{"turn":{"id":"turn-1"}}}),
    ]
}

fn call(tool: &str, arguments: Value) -> Value {
    json!({"id":"tool-1", "method":"item/tool/call", "params":{
        "threadId":"th-1", "turnId":"turn-1", "tool":tool, "namespace":null,
        "callId":"call-1", "arguments":arguments
    }})
}

#[test]
fn every_run_disables_other_tools_and_never_asks_for_broader_permissions() {
    for resume in [None, Some("th-1")] {
        let request = policy::thread(&session("ai-8", resume), &config()).unwrap();
        let params = &request["params"];
        assert_eq!(params["approvalPolicy"], "never");
        assert_eq!(params["sandbox"], "read-only");
        assert_eq!(params["model"], "chosen-model");
        for flag in DISABLED {
            assert_eq!(params["config"]["features"][flag], false);
        }
        assert_eq!(
            params["config"]["mcp_servers"]["foreign.server"]["enabled"],
            false
        );
        assert!(
            params["developerInstructions"]
                .as_str()
                .unwrap()
                .contains(crate::prompt::BRIEF)
        );
        if resume.is_none() {
            assert_eq!(request["method"], "thread/start");
            let tools = params["dynamicTools"].as_array().unwrap();
            assert_eq!(tools.len(), 1);
            assert_eq!(tools[0]["name"], "devcast");
            assert_eq!(params["environments"], json!([]));
        } else {
            assert_eq!(request["method"], "thread/resume");
            assert_eq!(params["threadId"], "th-1");
            assert_eq!(params["excludeTurns"], true);
        }
    }
}

#[test]
fn a_policy_that_did_not_take_effect_stops_before_starting_a_model_turn() {
    assert!(policy::thread(&session("", None), &config()).is_err());
    for flag in DISABLED {
        let mut unsafe_config = config();
        unsafe_config["features"][flag] = json!(true);
        assert!(
            policy::thread(&session("ai-1", None), &unsafe_config).is_err(),
            "{flag}"
        );
    }
    for server in [
        json!({"runtimeStatus":"ready", "tools":{}}),
        json!({"runtimeStatus":"disabled", "tools":{"run":{}}}),
    ] {
        assert!(policy::inventory(&json!({"data":[server]})).is_err());
    }
    assert!(policy::inventory(&json!({"data":[], "nextCursor":"more"})).is_err());
    assert!(policy::inventory(&json!({"data":[{"runtimeStatus":"disabled", "tools":{}}]})).is_ok());
}

#[test]
fn only_the_installed_devcast_can_run_and_shell_syntax_stays_literal_argv() {
    for group in ["ai-1", "ai-2"] {
        let mut state = ready(group);
        let args = json!(["text", "set", "a1", "--text", "шаг; & calc.exe $(whoami)"]);
        let batch = state
            .handle(&call("devcast", json!({"args":args})))
            .unwrap();
        let command = &batch.messages[0];
        assert_eq!(command["method"], "command/exec");
        assert_eq!(
            command["params"]["command"][0],
            "C:/Program Files/DevCast/devcast.exe"
        );
        assert_eq!(
            &command["params"]["command"].as_array().unwrap()[1..],
            args.as_array().unwrap().as_slice()
        );
        assert_eq!(command["params"]["cwd"], "C:/Видео/Урок 1.devcast");
        assert_eq!(command["params"]["env"], json!({"DEVCAST_GROUP":group}));
        assert_eq!(batch.events.len(), 1);
        let reply = state
            .handle(&json!({"id":100, "result":{
                "exitCode":0, "stdout":"готово", "stderr":""
            }}))
            .unwrap();
        assert_eq!(reply.messages[0]["id"], "tool-1");
        assert_eq!(reply.messages[0]["result"]["success"], true);
        assert_eq!(reply.events, vec![Event::Done { ok: true }]);
    }
}

#[test]
fn cancelling_before_the_command_event_is_accepted_never_sends_an_exec_request() {
    let mut messages = handshake();
    messages.push(call("devcast", json!({"args":["info"]})));
    let lines = messages
        .iter()
        .map(|message| format!("{message}\n"))
        .collect::<String>();
    let mut written = Vec::new();
    super::listen(
        session("ai-1", None),
        "монтаж".into(),
        &mut written,
        std::io::Cursor::new(lines),
        |event| !matches!(event, Event::Command(_)),
        || true,
    )
    .unwrap();
    let written = String::from_utf8(written).unwrap();
    assert!(written.contains("turn/start"));
    assert!(!written.contains("command/exec"));
}

#[test]
fn other_executables_files_permissions_and_context_substitution_are_rejected() {
    for request in [
        call("Bash", json!({"args":["devcast", "info"]})),
        call("cmd.exe", json!({"args":["/c", "whoami"]})),
        call(
            "devcast",
            json!({"args":["info"], "env":{"DEVCAST_GROUP":"other"}}),
        ),
        call("devcast", json!({"args":["info"], "cwd":"C:/other"})),
        call("devcast", json!({"args":["info"], "program":"cmd.exe"})),
        call("devcast", json!({"args":"devcast info"})),
        call("devcast", json!({"args":["info\u{0}"]})),
    ] {
        let batch = ready("ai-1").handle(&request).unwrap();
        assert!(batch.events.is_empty());
        assert_eq!(batch.messages.len(), 1);
        assert_eq!(batch.messages[0]["result"]["success"], false);
    }
    for method in [
        "command/exec",
        "item/fileChange/requestApproval",
        "item/permissions/requestApproval",
    ] {
        let batch = ready("ai-1")
            .handle(&json!({"id":7, "method":method, "params":{}}))
            .unwrap();
        assert!(batch.messages[0]["error"].is_object());
        assert!(batch.events.is_empty());
    }
    let mut request = call("devcast", json!({"args":["info"]}));
    request["params"]["turnId"] = json!("old-turn");
    assert_eq!(
        ready("ai-1").handle(&request).unwrap().messages[0]["result"]["success"],
        false
    );
}

#[test]
fn calls_before_the_policy_check_and_failed_commands_do_not_grant_permissions() {
    let mut state = State::new(session("ai-1", None), "монтаж".into());
    let denied = state
        .handle(&call("devcast", json!({"args":["info"]})))
        .unwrap();
    assert_eq!(denied.messages[0]["result"]["success"], false);
    for result in [
        json!({"result":{"exitCode":2, "stdout":"", "stderr":"bad argument"}}),
        json!({"error":{"message":"access denied"}}),
    ] {
        let mut state = ready("ai-1");
        state
            .handle(&call("devcast", json!({"args":["info"]})))
            .unwrap();
        let mut response = result;
        response["id"] = json!(100);
        let batch = state.handle(&response).unwrap();
        assert_eq!(batch.messages[0]["result"]["success"], false);
        assert_eq!(batch.events, vec![Event::Done { ok: false }]);
    }
}

#[cfg(windows)]
#[test]
fn native_process_hands_back_commands_usage_and_a_resumable_session() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock.ps1");
    std::fs::write(
        &script,
        include_str!("../../../../fixtures/agents/codex.ps1"),
    )
    .unwrap();
    let shim = tmp.path().join("codex.cmd");
    std::fs::write(&shim, include_str!("../../../../fixtures/agents/codex.cmd")).unwrap();
    let found = Found {
        agent: dc_settings::Agent::Codex,
        path: shim,
    };
    for resume in [None, Some("codex-9")] {
        let ask = Ask {
            found: &found,
            model: None,
            project: tmp.path(),
            devcast: Path::new("C:/DevCast"),
            system: Path::new("unused.md"),
            request: "добавь зум",
            resume,
            group: "ai-test",
        };
        let job = Job::start(crate::launch(&ask), || {}).unwrap();
        let events: Vec<_> = std::iter::from_fn(|| job.next()).collect();
        assert!(
            events.contains(&Event::Session("codex-9".into())),
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Command(_)))
                .count(),
            1
        );
        assert!(events.contains(&Event::Done { ok: true }), "{events:?}");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Usage(u) if u.work() == 15))
        );
        assert_eq!(
            events.last(),
            Some(&Event::Finished(Ok("Добавил зумы.".into())))
        );
    }
}
