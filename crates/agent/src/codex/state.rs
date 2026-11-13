//! A client state machine: no command is accepted before policy and tool inventory checks.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{Session, policy};
use crate::{Event, Usage};

#[derive(Default)]
pub(super) struct Batch {
    pub messages: Vec<Value>,
    pub events: Vec<Event>,
    pub done: bool,
}

pub(super) struct State {
    session: Session,
    request: String,
    expected: Option<u64>,
    thread: String,
    turn: String,
    ready: bool,
    next: u64,
    pending: BTreeMap<u64, Value>,
    said: String,
    model: Option<String>,
    baseline: Option<Usage>,
    usage: Option<Usage>,
}

impl State {
    pub fn new(session: Session, request: String) -> Self {
        Self {
            session,
            request,
            expected: Some(1),
            thread: String::new(),
            turn: String::new(),
            ready: false,
            next: 100,
            pending: BTreeMap::new(),
            said: String::new(),
            model: None,
            baseline: None,
            usage: None,
        }
    }

    pub fn handle(&mut self, message: &Value) -> Result<Batch, String> {
        if message.get("method").is_some() {
            return if message.get("id").is_some() {
                self.tool(message)
            } else {
                self.notification(message)
            };
        }
        self.response(message)
    }

    fn response(&mut self, message: &Value) -> Result<Batch, String> {
        let mut batch = Batch::default();
        let Some(id) = message["id"].as_u64() else {
            return Ok(batch);
        };
        if let Some(tool_id) = self.pending.remove(&id) {
            let result = &message["result"];
            let success = result["exitCode"].as_i64() == Some(0) && message["error"].is_null();
            let text = if message["error"].is_null() {
                format!("{}{}", text(&result["stdout"]), text(&result["stderr"]))
            } else {
                text(&message["error"]["message"]).to_owned()
            };
            batch
                .messages
                .push(policy::tool_result(tool_id, success, text));
            batch.events.push(Event::Done { ok: success });
            return Ok(batch);
        }
        if self.expected != Some(id) {
            return Ok(batch);
        }
        if !message["error"].is_null() {
            return Err(format!(
                "Codex: {}. Обнови Codex или начни новый разговор",
                text(&message["error"]["message"])
            ));
        }
        self.expected = None;
        let result = &message["result"];
        match id {
            1 => {
                batch
                    .messages
                    .push(json!({"method":"initialized", "params":{}}));
                batch
                    .messages
                    .push(json!({"id":2, "method":"config/read", "params":{
                        "cwd":self.session.project, "includeLayers":false
                    }}));
            }
            2 => batch
                .messages
                .push(policy::thread(&self.session, &result["config"])?),
            3 => self.thread(result, &mut batch)?,
            4 => {
                policy::inventory(result)?;
                batch
                    .messages
                    .push(json!({"id":5, "method":"turn/start", "params":{
                        "threadId":self.thread, "environments":[], "approvalPolicy":"never",
                        "sandboxPolicy":{"type":"readOnly", "networkAccess":false},
                        "input":[{"type":"text", "text":self.request}]
                    }}));
            }
            5 => {
                self.turn = required(&result["turn"]["id"], "id хода работы")?;
                self.ready = true;
            }
            _ => {}
        }
        if id < 5 {
            self.expected = Some(id + 1);
        }
        Ok(batch)
    }

    fn thread(&mut self, result: &Value, batch: &mut Batch) -> Result<(), String> {
        if result["approvalPolicy"] != "never" || result["sandbox"]["type"] != "readOnly" {
            return Err(
                "Codex не применил read-only и запрет разрешений: монтаж не запущен".into(),
            );
        }
        self.thread = required(&result["thread"]["id"], "id разговора")?;
        self.model = result["model"].as_str().map(str::to_owned);
        batch.events.push(Event::Session(self.thread.clone()));
        batch
            .messages
            .push(json!({"id":4, "method":"mcpServerStatus/list", "params":{
                "threadId":self.thread
            }}));
        Ok(())
    }

    fn tool(&mut self, message: &Value) -> Result<Batch, String> {
        let mut batch = Batch::default();
        let id = message["id"].clone();
        let params = &message["params"];
        let scoped =
            self.ready && params["threadId"] == self.thread && params["turnId"] == self.turn;
        if message["method"] != "item/tool/call" {
            batch.messages.push(json!({"id":id, "error":{
                "code":-32601, "message":"DevCast разрешает только инструмент devcast"
            }}));
            return Ok(batch);
        }
        let command = if scoped {
            policy::command(&self.session, params, self.next)
        } else {
            Err("вызов не относится к текущему запуску DevCast".to_owned())
        };
        match command {
            Ok(command) => {
                let argv = command["params"]["command"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .skip(1)
                    .map(|arg| format!("{:?}", text(arg)))
                    .collect::<Vec<_>>();
                batch
                    .events
                    .push(Event::Command(format!("devcast {}", argv.join(" "))));
                self.pending.insert(self.next, id);
                self.next = self
                    .next
                    .checked_add(1)
                    .ok_or("слишком много команд Codex")?;
                batch.messages.push(command);
            }
            Err(error) => batch.messages.push(policy::tool_result(id, false, error)),
        }
        Ok(batch)
    }

    fn notification(&mut self, message: &Value) -> Result<Batch, String> {
        let mut batch = Batch::default();
        let params = &message["params"];
        if params["threadId"] != self.thread || self.thread.is_empty() {
            return Ok(batch);
        }
        match text(&message["method"]) {
            "turn/started" if self.expected == Some(5) => {
                self.turn = required(&params["turn"]["id"], "id хода работы")?;
                self.ready = true;
            }
            "item/completed" if self.scoped(params) && params["item"]["type"] == "agentMessage" => {
                self.said = text(&params["item"]["text"]).to_owned();
                batch.events.push(Event::Said(self.said.clone()));
            }
            "thread/tokenUsage/updated" if self.scoped(params) => {
                self.counts(&params["tokenUsage"])
            }
            "turn/completed" if params["turn"]["id"] == self.turn && self.ready => {
                if !self.pending.is_empty() {
                    return Err("Codex завершился до ответа команды".into());
                }
                if let Some(usage) = &self.usage {
                    batch.events.push(Event::Usage(usage.clone()));
                }
                let result = if params["turn"]["status"] == "completed" {
                    Ok(self.said.clone())
                } else {
                    Err(format!(
                        "Codex: {}",
                        text(&params["turn"]["error"]["message"])
                    ))
                };
                batch.events.push(Event::Finished(result));
                batch.done = true;
                self.ready = false;
            }
            _ => {}
        }
        Ok(batch)
    }

    fn scoped(&self, params: &Value) -> bool {
        self.ready && params["turnId"] == self.turn
    }

    fn counts(&mut self, counts: &Value) {
        let read = |counts: &Value| Usage {
            model: self.model.clone(),
            input: counts["inputTokens"]
                .as_u64()
                .unwrap_or(0)
                .saturating_sub(counts["cachedInputTokens"].as_u64().unwrap_or(0)),
            output: counts["outputTokens"].as_u64().unwrap_or(0),
            cache_read: counts["cachedInputTokens"].as_u64().unwrap_or(0),
            cache_write: counts["cacheWriteInputTokens"].as_u64().unwrap_or(0),
        };
        let total = read(&counts["total"]);
        let baseline = self
            .baseline
            .get_or_insert_with(|| subtract(&total, &read(&counts["last"])));
        self.usage = Some(subtract(&total, baseline));
    }
}

fn subtract(total: &Usage, baseline: &Usage) -> Usage {
    Usage {
        model: total.model.clone(),
        input: total.input.saturating_sub(baseline.input),
        output: total.output.saturating_sub(baseline.output),
        cache_read: total.cache_read.saturating_sub(baseline.cache_read),
        cache_write: total.cache_write.saturating_sub(baseline.cache_write),
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

fn required(value: &Value, field: &str) -> Result<String, String> {
    value
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("Codex не вернул {field}: обнови Codex и повтори запуск"))
}
