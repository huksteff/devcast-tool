//! Minidumps of native crashes (crash-handler + minidumper): the app starts a copy of itself
//! with `--crash-server`, and a crash of the app asks that process to write the dump of the
//! crashed one into `%APPDATA%\DevCast\logs\crashes`. The only module of dc-app with `unsafe`
//! (owner's decision): crash-handler's crash callback is an `unsafe` contract.
//!
//! The server lives exactly as long as its parent: the parent holds the write end of the
//! server's stdin and never writes, so the server reads end-of-file once the parent is gone,
//! however it ended (closed, killed, crashed after the dump) and even if it never connected.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crash_handler::{CrashEventResult, CrashHandler};

use crate::error::AppError;

/// Argument that turns the program into the dump writer.
pub const SERVER_ARG: &str = "--crash-server";
/// What the server prints once it listens.
const READY: &str = "ready";

/// Keeps the dump writer and the handler alive for the life of the app; dropping it closes
/// the server's stdin, which ends the server.
pub struct Guard {
    _handler: CrashHandler,
    _server: Child,
}

/// Starts the dump writer into the folder `crashes` and attaches the crash handler.
pub fn install(crashes: &Path) -> Result<Guard, AppError> {
    let error = |e: &dyn std::fmt::Display| AppError::Crash(e.to_string());
    let socket = std::env::temp_dir().join(format!("devcast-crash-{}", std::process::id()));
    let exe = std::env::current_exe().map_err(|e| error(&e))?;
    let mut server = Command::new(exe)
        .arg(SERVER_ARG)
        .arg(&socket)
        .arg(crashes)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| error(&e))?;
    let stdout = server
        .stdout
        .take()
        .ok_or_else(|| error(&"нет вывода процесса дампов"))?;
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .map_err(|e| error(&e))?;
    if line.trim() != READY {
        return Err(error(&"процесс дампов не запустился"));
    }
    let client = minidumper::Client::with_name(socket.as_path()).map_err(|e| error(&e))?;
    // SAFETY: the callback only asks the other process for a dump over an already open socket;
    // it allocates nothing and touches no locks of this process (crash-handler's contract).
    let event = unsafe {
        crash_handler::make_crash_event(move |context| {
            CrashEventResult::Handled(client.request_dump(context).is_ok())
        })
    };
    let handler = CrashHandler::attach(event).map_err(|e| error(&e))?;
    Ok(Guard {
        _handler: handler,
        _server: server,
    })
}

/// The dump writer: listens on `socket`, writes dumps into `dir`, ends with its client or
/// once `parent` (the stdin from the app) reaches its end.
pub fn serve(
    socket: &Path,
    dir: PathBuf,
    parent: impl Read + Send + 'static,
) -> Result<(), AppError> {
    let error = |e: &dyn std::fmt::Display| AppError::Crash(e.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| AppError::settings_file(&dir, e))?;
    let mut server = minidumper::Server::with_name(socket).map_err(|e| error(&e))?;
    let mut out = std::io::stdout();
    writeln!(out, "{READY}")
        .and_then(|()| out.flush())
        .map_err(|e| error(&e))?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let gone = Arc::clone(&shutdown);
    std::thread::Builder::new()
        .name("devcast-crash-parent".to_owned())
        .spawn(move || {
            let mut parent = parent;
            // The parent never writes: data, end or a broken pipe all mean it is gone.
            let _ignored = parent.read(&mut [0; 1]);
            gone.store(true, Ordering::Relaxed);
        })
        .map_err(|e| error(&e))?;
    server
        .run(Box::new(Dumps { dir }), &shutdown, None)
        .map_err(|e| error(&e))
}

struct Dumps {
    dir: PathBuf,
}

impl minidumper::ServerHandler for Dumps {
    fn create_minidump_file(&self) -> Result<(std::fs::File, PathBuf), std::io::Error> {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = self.dir.join(format!("devcast-app-{seconds}.dmp"));
        Ok((std::fs::File::create(&path)?, path))
    }

    fn on_minidump_created(
        &self,
        result: Result<minidumper::MinidumpBinary, minidumper::Error>,
    ) -> minidumper::LoopAction {
        if let Err(error) = result {
            eprintln!("дамп падения не записан: {error}");
        }
        minidumper::LoopAction::Exit
    }

    fn on_message(&self, _kind: u32, _buffer: Vec<u8>) {}

    fn on_client_disconnected(&self, clients: usize) -> minidumper::LoopAction {
        if clients == 0 {
            minidumper::LoopAction::Exit
        } else {
            minidumper::LoopAction::Continue
        }
    }
}
