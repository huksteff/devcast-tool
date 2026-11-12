//! Codex's picker catalog, read without starting a conversation or an inference turn.

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Found, codex};

/// A picker-visible model. The model slug, rather than its display name, goes to Codex.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub model: String,
    pub display_name: String,
    #[serde(default)]
    hidden: bool,
}

/// A cancellable catalog request; drop also stops an unresponsive server.
#[derive(Debug)]
pub struct Catalog {
    child: Child,
    answer: Receiver<io::Result<Vec<Model>>>,
}

impl Catalog {
    pub fn start(found: &Found, wake: impl FnOnce() + Send + 'static) -> io::Result<Self> {
        let mut command = Command::new(&found.path);
        command
            .args(codex::server_args())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        crate::run::no_window(&mut command);
        let mut child = command.spawn()?;
        let pipes = child.stdin.take().zip(child.stdout.take());
        let Some((input, output)) = pipes else {
            crate::run::kill_tree(&mut child);
            let _ = child.wait();
            return Err(io::Error::other("Codex не открыл канал списка моделей"));
        };
        let (send, answer) = channel();
        let started = std::thread::Builder::new()
            .name("devcast-models".into())
            .spawn(move || {
                let _ = send.send(read(input, BufReader::new(output)));
                wake();
            });
        if let Err(error) = started {
            crate::run::kill_tree(&mut child);
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self { child, answer })
    }

    pub fn poll(&self) -> Option<io::Result<Vec<Model>>> {
        match self.answer.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(io::Error::other(
                "Codex закрыл канал списка моделей: обнови Codex и повтори проверку",
            ))),
        }
    }
}

impl Drop for Catalog {
    fn drop(&mut self) {
        crate::run::kill_tree(&mut self.child);
        let _ = self.child.wait();
    }
}

fn read(mut input: impl Write, mut output: impl BufRead) -> io::Result<Vec<Model>> {
    codex::send(
        &mut input,
        &json!({"id":1,"method":"initialize","params":{
            "clientInfo":{"name":"devcast","version":env!("CARGO_PKG_VERSION")}
        }}),
    )?;
    response(&mut output, 1)?;
    codex::send(&mut input, &json!({"method":"initialized"}))?;
    let mut models = Vec::new();
    let mut cursor = None::<String>;
    let mut seen = std::collections::HashSet::new();
    let mut id = 2;
    loop {
        codex::send(
            &mut input,
            &json!({"id":id,"method":"model/list","params":{
                "limit":100,"includeHidden":false,"cursor":cursor
            }}),
        )?;
        let result = response(&mut output, id)?;
        let page: Vec<Model> = serde_json::from_value(result["data"].clone())?;
        for model in page
            .into_iter()
            .filter(|m| !m.hidden && !m.model.is_empty())
        {
            if !models.iter().any(|m: &Model| m.model == model.model) {
                models.push(model);
            }
        }
        cursor = result["nextCursor"].as_str().map(str::to_owned);
        let Some(next) = &cursor else {
            return Ok(models);
        };
        if !seen.insert(next.clone()) {
            return Err(io::Error::other("Codex повторил страницу списка моделей"));
        }
        id += 1;
    }
}

fn response(output: &mut impl BufRead, id: u64) -> io::Result<Value> {
    while let Some(message) = codex::receive(output)? {
        if message["id"].as_u64() != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(io::Error::other(format!("Codex: {error}")));
        }
        return message
            .get("result")
            .cloned()
            .ok_or_else(|| io::Error::other("Codex не вернул список моделей"));
    }
    Err(io::Error::other(
        "Codex закрыл соединение: обнови Codex и повтори проверку",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_uses_slugs_and_visible_models_from_every_page_without_a_turn() {
        let replies = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"method\":\"notice\"}\n",
            "{\"id\":2,\"result\":{\"data\":[{\"model\":\"first\",\"displayName\":\"First\"},{\"model\":\"hidden\",\"displayName\":\"Hidden\",\"hidden\":true}],\"nextCursor\":\"page2\"}}\n",
            "{\"id\":3,\"result\":{\"data\":[{\"model\":\"second\",\"displayName\":\"Second\"}],\"nextCursor\":null}}\n"
        );
        let mut sent = Vec::new();
        let models = read(&mut sent, io::Cursor::new(replies)).unwrap();
        assert_eq!(
            models.iter().map(|m| m.model.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(models[1].display_name, "Second");
        let sent = String::from_utf8(sent).unwrap();
        assert!(sent.contains("page2"));
        assert!(!sent.contains("thread/start") && !sent.contains("turn/start"));
    }

    #[test]
    fn broken_catalog_is_an_error_instead_of_an_empty_success() {
        for replies in [
            "",
            "{\"id\":1,\"error\":{\"message\":\"unsupported\"}}\n",
            "{\"id\":1,\"result\":{}}\n{\"id\":2,\"result\":{}}\n",
        ] {
            assert!(read(Vec::new(), io::Cursor::new(replies)).is_err());
        }
    }
}
