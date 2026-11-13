//! Claude Code's `--output-format stream-json` lines as [`Event`]s.
//! A line that is not part of the protocol is skipped.

use serde_json::Value;

use crate::usage::{Limits, Usage};

/// What the agent did, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The conversation's id, to go on with it (`--resume`).
    Session(String),
    /// It runs a DevCast command.
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

/// The events of one Claude Code output line.
pub fn line(line: &str) -> Vec<Event> {
    let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
        return Vec::new();
    };
    claude(&value)
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
                true if said.is_empty() => failed(&errors(value)),
                true => failed(&said),
            };
            let usage = Usage::claude(value).map(Event::Usage);
            usage.into_iter().chain([end]).collect()
        }
        _ => Vec::new(),
    }
}

/// Why a run failed before its first turn (`errors` of the result: an expired sign-in).
fn errors(value: &Value) -> String {
    let lines = value["errors"].as_array().cloned().unwrap_or_default();
    let lines: Vec<&str> = lines.iter().map(text).collect();
    lines.join(" ")
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
