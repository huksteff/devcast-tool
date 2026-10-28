//! Control of a running `devcast record` from another `devcast` call (decision 011): the
//! recording process listens on the channel `devcast-record-<user>`; `devcast record stop`
//! (`pause`, `resume`, `cancel`) sends one request and prints the answer, which comes once the
//! recording has done it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use dc_ipc::{Connection, IpcError, Listener};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::CliError;
use crate::run::Answer;

/// What another call asks of the running recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Control {
    Stop,
    Pause,
    Resume,
    Cancel,
}

impl Control {
    fn verb(self) -> &'static str {
        match self {
            Self::Stop => "остановить",
            Self::Pause => "поставить на паузу",
            Self::Resume => "продолжить",
            Self::Cancel => "отменить (удалить файлы)",
        }
    }
}

/// The recording's answer: what `devcast record <control>` prints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub(crate) enum Reply {
    Done { text: String, json: Value },
    Failed { message: String },
}

impl Reply {
    pub(crate) fn of(result: &Result<Answer, CliError>) -> Self {
        match result {
            Ok(answer) => Self::Done {
                text: answer.text.clone(),
                json: answer.json.clone(),
            },
            Err(error) => Self::Failed {
                message: error.to_string(),
            },
        }
    }
}

/// A request handed to the recording, with the way back to the caller.
pub(crate) struct Request {
    pub control: Control,
    pub reply: Sender<Reply>,
}

fn channel() -> Result<String, CliError> {
    dc_ipc::socket_name("record").map_err(|e| CliError::Failed(e.to_string()))
}

/// `devcast record <control>`.
pub(crate) fn send(control: Control, dry_run: bool) -> Result<Answer, CliError> {
    if dry_run {
        let text = format!(
            "dry-run: запись не тронута; команда — {} её",
            control.verb()
        );
        return Ok(Answer::new(
            text,
            json!({"dry_run": true, "control": control}),
        ));
    }
    match dc_ipc::request(&channel()?, &control) {
        Ok(Reply::Done { text, json }) => Ok(Answer::new(text, json)),
        Ok(Reply::Failed { message }) => Err(CliError::Failed(message)),
        Err(IpcError::NotRunning(_)) => Err(CliError::Failed(
            "запись не идёт: начни её — devcast record --source screen:1 (источники — devcast ls sources)"
                .to_owned(),
        )),
        Err(e) => Err(CliError::Failed(format!(
            "не удалось связаться с записью ({e}): проверь, что процесс devcast record жив"
        ))),
    }
}

/// The listening side, in the recording process. It takes the channel before the recording
/// starts, so a second recording cannot start; requests that come while the recording starts
/// wait in [`Self::requests`].
pub(crate) struct Server {
    name: String,
    closing: Arc<AtomicBool>,
    thread: JoinHandle<Vec<String>>,
    pub requests: Receiver<Request>,
}

impl Server {
    pub(crate) fn start() -> Result<Self, CliError> {
        let name = channel()?;
        let listener = Listener::bind(&name).map_err(|e| {
            CliError::Failed(format!(
                "{e}: запись уже идёт — останови её (devcast record stop) или отмени (devcast record cancel)"
            ))
        })?;
        let (requests, received) = mpsc::channel();
        let closing = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&closing);
        let thread = std::thread::Builder::new()
            .name("devcast-record-control".to_owned())
            .spawn(move || serve(&listener, &requests, &flag))
            .map_err(|e| {
                CliError::Failed(format!("не удалось запустить управление записью: {e}"))
            })?;
        Ok(Self {
            name,
            closing,
            thread,
            requests: received,
        })
    }

    /// Frees the channel; returns what went wrong with requests on the way, as warnings.
    pub(crate) fn close(self) -> Result<Vec<String>, CliError> {
        self.closing.store(true, Ordering::SeqCst);
        drop(self.requests);
        Listener::wake(&self.name).map_err(|e| {
            CliError::Failed(format!("не удалось закрыть канал управления записью: {e}"))
        })?;
        self.thread
            .join()
            .map_err(|_| CliError::Failed("поток управления записью упал".to_owned()))
    }
}

fn serve(listener: &Listener, requests: &Sender<Request>, closing: &AtomicBool) -> Vec<String> {
    let mut warnings = Vec::new();
    loop {
        let connection = listener.accept();
        if closing.load(Ordering::SeqCst) {
            return warnings;
        }
        match connection {
            Ok(mut connection) => {
                if let Err(e) = answer(&mut connection, requests) {
                    warnings.push(format!("запрос управления записью не обработан: {e}"));
                }
            }
            Err(e) => {
                warnings.push(format!(
                    "канал управления записью закрылся ({e}): stop/pause больше не дойдут"
                ));
                return warnings;
            }
        }
    }
}

fn answer(connection: &mut Connection, requests: &Sender<Request>) -> Result<(), IpcError> {
    let control: Control = connection.receive()?;
    let (reply, replied) = mpsc::channel();
    let ended = || Reply::Failed {
        message: "запись уже закончилась".to_owned(),
    };
    let answer = match requests.send(Request { control, reply }) {
        Ok(()) => replied.recv().unwrap_or_else(|_| ended()),
        Err(_) => ended(),
    };
    connection.send(&answer)
}
