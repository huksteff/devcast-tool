//! The export queue between CLI calls (decision 011, «Задача 5.14»): the first `devcast export`
//! takes the channel `devcast-export-<user>` and runs `dc_encode::ExportQueue`; later calls
//! send their export there and wait for its answer; `export status` and `export cancel` ask it.
//! The process that holds the channel lives until the queue is empty.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use dc_encode::{ExportQueue, JobState};
use dc_ipc::{IpcError, Listener};
use dc_ops::export::Spec;
use dc_render::Gpu;
use serde::{Deserialize, Serialize};

use crate::error::CliError;
pub(crate) use dc_ops::export_jobs::{Finished, Job, finished};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub(crate) enum Request {
    /// Queue this export; the answer comes when it has finished.
    Run(Box<Spec>),
    Status,
    /// Cancel one job or (`None`) all of them.
    Cancel {
        id: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub(crate) enum Reply {
    Finished(Finished),
    Status { jobs: Vec<Job> },
    Cancelled { ids: Vec<u32> },
}

/// Sends `request` to the process that runs the queue; `None` when there is none.
pub(crate) fn ask(channel: &str, request: &Request) -> Result<Option<Reply>, CliError> {
    match dc_ipc::request(channel, request) {
        Ok(reply) => Ok(Some(reply)),
        Err(IpcError::NotRunning(_)) => Ok(None),
        Err(e) => Err(CliError::Failed(format!(
            "очередь экспорта не ответила ({e}): проверь, что процесс devcast export жив"
        ))),
    }
}

/// The queue of this process and the channel that feeds it.
pub(crate) struct Server {
    pub queue: Arc<ExportQueue>,
    gpu: Arc<Gpu>,
    channel: String,
    closing: Arc<AtomicBool>,
    thread: JoinHandle<Vec<String>>,
}

impl Server {
    /// Takes `channel`; `None` when another process holds it.
    pub(crate) fn start(channel: &str, gpu: Gpu) -> Result<Option<Self>, CliError> {
        let listener = match Listener::bind(channel) {
            Ok(listener) => listener,
            Err(IpcError::Taken { .. }) => return Ok(None),
            Err(e) => return Err(CliError::Failed(e.to_string())),
        };
        let queue = Arc::new(ExportQueue::new().map_err(|e| CliError::Failed(e.to_string()))?);
        let gpu = Arc::new(gpu);
        let closing = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("devcast-export-channel".to_owned())
            .spawn({
                let (queue, gpu, closing) = (queue.clone(), gpu.clone(), closing.clone());
                move || serve(&listener, &queue, &gpu, &closing)
            })
            .map_err(|e| CliError::Failed(format!("не удалось запустить очередь экспорта: {e}")))?;
        Ok(Some(Self {
            queue,
            gpu,
            channel: channel.to_owned(),
            closing,
            thread,
        }))
    }

    pub(crate) fn push(&self, spec: Spec) -> u32 {
        push(&self.queue, &self.gpu, spec)
    }

    /// Waits until the queue is empty and frees the channel. Exports that came in meanwhile
    /// finish first: the channel thread waits for their answers. Returns problems with
    /// requests on the way, as warnings.
    pub(crate) fn close(self) -> Result<Vec<String>, CliError> {
        self.queue.wait_idle();
        self.closing.store(true, Ordering::SeqCst);
        Listener::wake(&self.channel).map_err(|e| {
            CliError::Failed(format!("не удалось закрыть канал очереди экспорта: {e}"))
        })?;
        self.thread
            .join()
            .map_err(|_| CliError::Failed("поток очереди экспорта упал".to_owned()))
    }
}

fn push(queue: &ExportQueue, gpu: &Arc<Gpu>, spec: Spec) -> u32 {
    let gpu = Arc::clone(gpu);
    let label = spec.label();
    queue.push(
        label,
        Box::new(move |progress| dc_ops::export::run(&gpu, &spec, progress)),
    )
}

fn serve(
    listener: &Listener,
    queue: &Arc<ExportQueue>,
    gpu: &Arc<Gpu>,
    closing: &AtomicBool,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut waiting = Vec::new();
    loop {
        let connection = listener.accept();
        if closing.load(Ordering::SeqCst) {
            break;
        }
        let result = connection.and_then(|mut connection| {
            let request: Request = connection.receive()?;
            match request {
                Request::Run(spec) => {
                    let id = push(queue, gpu, *spec);
                    let queue = Arc::clone(queue);
                    waiting.push(std::thread::spawn(move || {
                        let state = queue.wait(id).unwrap_or(JobState::Cancelled);
                        connection.send(&Reply::Finished(finished(id, state)))
                    }));
                    Ok(())
                }
                Request::Status => {
                    let jobs = queue.status().iter().map(Job::of).collect();
                    connection.send(&Reply::Status { jobs })
                }
                Request::Cancel { id } => connection.send(&Reply::Cancelled {
                    ids: queue.cancel(id),
                }),
            }
        });
        if let Err(e) = result {
            warnings.push(format!("запрос к очереди экспорта не обработан: {e}"));
        }
    }
    for thread in waiting {
        match thread.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warnings.push(format!("ответ о готовом экспорте не дошёл: {e}")),
            Err(_) => warnings.push("поток ответа очереди экспорта упал".to_owned()),
        }
    }
    warnings
}
