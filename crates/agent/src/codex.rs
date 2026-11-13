//! Codex's native app-server protocol. The model gets one client-owned tool, not a shell.

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{Ask, Event};

mod policy;
mod state;

/// Features that could expose another executor, file access or user-provided commands.
const DISABLED: &[&str] = &[
    "shell_tool",
    "apps",
    "plugins",
    "hooks",
    "browser_use",
    "computer_use",
    "image_generation",
    "view_image",
    "multi_agent",
    "code_mode",
    "tool_suggest",
    "shell_snapshot",
    "sleep_tool",
    "skill_mcp_dependency_install",
    "artifact",
];

#[derive(Debug, Clone)]
pub(crate) struct Session {
    exe: PathBuf,
    project: PathBuf,
    group: String,
    model: Option<String>,
    resume: Option<String>,
}

pub(crate) enum End {
    Finished,
    Stopped,
}

pub(crate) fn configure(args: &mut Vec<OsString>, ask: &Ask<'_>) -> Session {
    args.extend(server_args());
    Session {
        exe: ask.devcast.join(if cfg!(windows) {
            "devcast.exe"
        } else {
            "devcast"
        }),
        project: ask.project.to_owned(),
        group: ask.group.to_owned(),
        model: ask.model.map(str::to_owned),
        resume: ask.resume.map(str::to_owned),
    }
}

pub(crate) fn server_args() -> Vec<OsString> {
    let mut args = Vec::new();
    args.extend(["app-server".into(), "--stdio".into()]);
    for flag in DISABLED {
        args.extend(["--config".into(), format!("features.{flag}=false").into()]);
    }
    args.extend(["--config".into(), "web_search=\"disabled\"".into()]);
    args
}

/// A bad protocol or an unsupported policy ends the run before any further tool is executed.
pub(crate) fn listen(
    session: Session,
    request: String,
    mut input: impl Write,
    mut output: impl BufRead,
    emit: impl Fn(Event) -> bool,
    running: impl Fn() -> bool,
) -> io::Result<End> {
    let mut state = state::State::new(session, request);
    send(
        &mut input,
        &json!({"id":1, "method":"initialize", "params":{
            "clientInfo":{"name":"devcast", "version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"experimentalApi":true}
        }}),
    )?;
    while let Some(message) = receive(&mut output)? {
        if !running() {
            return Ok(End::Stopped);
        }
        let batch = state.handle(&message).map_err(io::Error::other)?;
        for event in batch.events {
            if !emit(event) {
                return Ok(End::Stopped);
            }
        }
        for message in batch.messages {
            if !running() {
                return Ok(End::Stopped);
            }
            send(&mut input, &message)?;
        }
        if batch.done {
            return Ok(End::Finished);
        }
    }
    Err(io::Error::other(
        "Codex закрыл соединение без ответа: обнови Codex и повтори запуск",
    ))
}

pub(crate) fn send(writer: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

pub(crate) fn receive(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    use std::io::Read;
    const MAX: u64 = 16 << 20;
    let mut line = Vec::new();
    let size = reader.take(MAX + 1).read_until(b'\n', &mut line)?;
    if size == 0 {
        return Ok(None);
    }
    if size as u64 > MAX {
        return Err(io::Error::other(
            "слишком большое сообщение Codex: начни новый разговор",
        ));
    }
    serde_json::from_slice(&line).map(Some).map_err(Into::into)
}

#[cfg(test)]
#[path = "codex/tests.rs"]
mod tests;
