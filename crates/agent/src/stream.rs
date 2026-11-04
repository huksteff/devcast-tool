//! The agent's output lines as [`Event`]s: Claude Code `--output-format stream-json`, Codex
//! `exec --json`. A line that is not one of theirs is skipped.

use dc_settings::Agent;
use serde_json::Value;

use crate::usage::{Limits, Usage};

/// What the agent did, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The conversation's id, to go on with it (`--resume`).
    Session(String),
    /// It runs a shell command line.
    Command(String),
    /// The command ended; `ok` — without an error.
    Done { ok: bool },
    /// Its words to the user.
    Said(String),
    /// What the run cost; comes right before [`Event::Finished`] when the agent says.
    Usage(Usage),
    /// How full the subscription's limits are now (Claude Code says it during a run).
    Limits(Limits),
    /// The run ended: the agent's last words, or why it failed. Nothing comes after it.
    Finished(Result<String, String>),
}

/// Reads one agent's lines; Codex tells its last words before the end of the turn, so the
/// reader keeps them.
#[derive(Debug)]
pub struct Reader {
    agent: Agent,
    last_said: String,
}

impl Reader {
    pub fn new(agent: Agent) -> Self {
        Self {
            agent,
            last_said: String::new(),
        }
    }

    /// The events of one output line.
    pub fn line(&mut self, line: &str) -> Vec<Event> {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        let events = match self.agent {
            Agent::Claude => claude(&value),
            Agent::Codex => self.codex(&value),
        };
        for event in &events {
            if let Event::Said(text) = event {
                self.last_said.clone_from(text);
            }
        }
        events
    }

    fn codex(&self, value: &Value) -> Vec<Event> {
        let item = &value["item"];
        match (text(&value["type"]), text(&item["type"])) {
            ("thread.started", _) => opt(text(&value["thread_id"]), Event::Session),
            ("item.started", "command_execution") => opt(text(&item["command"]), Event::Command),
            ("item.completed", "command_execution") => {
                let ok = item["exit_code"].as_i64() == Some(0);
                vec![Event::Done { ok }]
            }
            ("item.completed", "agent_message") => opt(text(&item["text"]), Event::Said),
            ("turn.completed", _) => {
                let usage = Usage::codex(value).map(Event::Usage);
                let end = Event::Finished(Ok(self.last_said.clone()));
                usage.into_iter().chain([end]).collect()
            }
            ("turn.failed", _) => vec![failed(text(&value["error"]["message"]))],
            ("error", _) => vec![failed(text(&value["message"]))],
            _ => Vec::new(),
        }
    }
}

fn claude(value: &Value) -> Vec<Event> {
    let content = || {
        value["message"]["content"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    match text(&value["type"]) {
        "system" if text(&value["subtype"]) == "init" => {
            opt(text(&value["session_id"]), Event::Session)
        }
        "assistant" => content()
            .iter()
            .filter_map(|part| match text(&part["type"]) {
                "text" => Some(Event::Said(text(&part["text"]).trim().to_owned())),
                "tool_use" => Some(Event::Command(command(&part["input"]))),
                _ => None,
            })
            .filter(|e| !matches!(e, Event::Said(s) | Event::Command(s) if s.is_empty()))
            .collect(),
        "rate_limit_event" => Limits::claude(value)
            .map(Event::Limits)
            .into_iter()
            .collect(),
        "user" => content()
            .iter()
            .filter(|part| text(&part["type"]) == "tool_result")
            .map(|part| Event::Done {
                ok: !part["is_error"].as_bool().unwrap_or(false),
            })
            .collect(),
        "result" => {
            let said = text(&value["result"]).trim().to_owned();
            let end = match value["is_error"].as_bool().unwrap_or(false) {
                false => Event::Finished(Ok(said)),
                true => failed(&said),
            };
            let usage = Usage::claude(value).map(Event::Usage);
            usage.into_iter().chain([end]).collect()
        }
        _ => Vec::new(),
    }
}

/// The command line of a tool call: `Bash` has `command`; any other tool shows its name.
fn command(input: &Value) -> String {
    match input["command"].as_str() {
        Some(line) => line.trim().to_owned(),
        None => input.to_string(),
    }
}

fn failed(why: &str) -> Event {
    let why = if why.trim().is_empty() {
        "нейронка остановилась с ошибкой без объяснения"
    } else {
        why.trim()
    };
    Event::Finished(Err(why.to_owned()))
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

fn opt(value: &str, event: fn(String) -> Event) -> Vec<Event> {
    match value.is_empty() {
        true => Vec::new(),
        false => vec![event(value.to_owned())],
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
pub(crate) mod tests;
