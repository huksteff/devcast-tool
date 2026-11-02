//! The app's export queue for `devcast export`: an export of the project the editor holds joins
//! it (`Request::Export`), so the editor's export list, its toasts and `export status` show one
//! queue and two exports never compete for the device. The queue of `devcast export` processes
//! (`export_queue.rs`) stays for projects the app does not hold. Ids name their queue: `e2` — of
//! `devcast export`, `p2` — of the app (`a…` are annotations).

use dc_ops::export::Spec;
use dc_ops::export_jobs::{Finished, Job};
use dc_ops::live::{Answer, Request};

use crate::error::CliError;
use crate::live::{ask_app, refused};

/// Where a job runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Queue {
    /// The queue of `devcast export` processes.
    Cli,
    /// The app's queue.
    App,
}

impl Queue {
    /// `e2`, `p2`.
    pub(crate) fn tag(self, id: u32) -> String {
        match self {
            Self::Cli => format!("e{id}"),
            Self::App => format!("p{id}"),
        }
    }

    /// The value of `queue` in JSON.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::App => "app",
        }
    }
}

/// `e2` or `p2`.
pub(crate) fn parse_id(text: &str) -> Result<(Queue, u32), CliError> {
    let queue = match text.chars().next() {
        Some('e') => Some(Queue::Cli),
        Some('p') => Some(Queue::App),
        _ => None,
    };
    let number = text.get(1..).and_then(|n| n.parse().ok());
    queue.zip(number).ok_or_else(|| {
        CliError::Usage(format!(
            "«{text}» — не номер экспорта: e1, e2 … или p1, p2 … (devcast export status)"
        ))
    })
}

/// Runs `spec` in the queue of the app on `channel` and waits for its end; `None` when the app
/// has no queue (or is gone): the run exports by itself.
pub(crate) fn run(channel: &str, spec: &Spec) -> Result<Option<Finished>, CliError> {
    match ask_app(channel, &Request::Export(Box::new(spec.clone())))? {
        Some(Answer::Exported(finished)) => Ok(Some(finished)),
        Some(Answer::NoExports) | None => Ok(None),
        Some(answer) => Err(refused(answer)),
    }
}

/// The jobs of the app's queue; none when the app is not running.
pub(crate) fn jobs(channel: &str) -> Result<Vec<Job>, CliError> {
    match ask_app(channel, &Request::Exports)? {
        Some(Answer::Exports { jobs }) => Ok(jobs),
        None => Ok(Vec::new()),
        Some(answer) => Err(refused(answer)),
    }
}

/// Cancels job `id` of the app's queue or all of them; the ids cancelled.
pub(crate) fn cancel(channel: &str, id: Option<u32>) -> Result<Vec<u32>, CliError> {
    match ask_app(channel, &Request::CancelExports { id })? {
        Some(Answer::ExportsCancelled { ids }) => Ok(ids),
        None => Ok(Vec::new()),
        Some(answer) => Err(refused(answer)),
    }
}
