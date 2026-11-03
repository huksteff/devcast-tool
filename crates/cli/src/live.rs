//! `devcast` on a project the editor holds (7.32, `dc_ops::live`): the run reads the working
//! state the editor has just put on disk, does its command as without the editor, and sends what
//! it did to the editor instead of writing files. The editor is asked only when
//! `lock-owner.json` names the app; a stale name (the app is gone or has closed the project)
//! leaves the run to the files.

use std::path::PathBuf;

use dc_ipc::IpcError;
use dc_ops::live::{Answer, LOCK_OWNER, Request, SERVICE};
use dc_project::history_log::Event;
use dc_project::{LockOwner, ProjectDir};

use crate::error::CliError;

/// The editor that holds the project of this run.
#[derive(Debug)]
pub(crate) struct Editor {
    channel: String,
    root: PathBuf,
    /// The editor's working state the run has read, or the one after the run's last write.
    revision: u64,
}

impl Editor {
    /// The editor holding `dir`, asked on `channel` (`None` — the real `devcast-app-<user>`); its
    /// working state is on disk when it answers. `None` when no editor has the project.
    pub(crate) fn open(dir: &ProjectDir, channel: Option<&str>) -> Result<Option<Self>, CliError> {
        if LockOwner::of(dir).is_none_or(|owner| owner.app != LOCK_OWNER) {
            return Ok(None);
        }
        let channel = match channel {
            Some(channel) => channel.to_owned(),
            None => dc_ipc::socket_name(SERVICE).map_err(lost)?,
        };
        let root = dir.root().to_path_buf();
        let asked = dc_ipc::request(&channel, &Request::Open { root: root.clone() });
        match asked {
            Ok(Answer::Open { revision }) => Ok(Some(Self {
                channel,
                root,
                revision,
            })),
            Ok(Answer::NotOpen) | Err(IpcError::NotRunning(_)) => Ok(None),
            Ok(answer) => Err(refused(answer)),
            Err(error) => Err(lost(error)),
        }
    }

    /// Sends `events`, which this run made of the state it read: the editor applies them as one
    /// step of its history.
    pub(crate) fn write(&mut self, events: &[Event]) -> Result<(), CliError> {
        let request = Request::Write {
            root: self.root.clone(),
            revision: self.revision,
            events: events.to_vec(),
        };
        match self.ask(&request)? {
            Answer::Written { revision } => {
                self.revision = revision;
                Ok(())
            }
            answer => Err(refused(answer)),
        }
    }

    /// `devcast font add` copied a font into the project: the editor lists it in its pickers.
    pub(crate) fn fonts_added(&self) -> Result<(), CliError> {
        let request = Request::Fonts {
            root: self.root.clone(),
        };
        match self.ask(&request)? {
            Answer::FontsRead => Ok(()),
            answer => Err(refused(answer)),
        }
    }

    /// The channel of the app whose editor this is.
    pub(crate) fn channel(&self) -> &str {
        &self.channel
    }

    /// `devcast save` through the editor.
    pub(crate) fn save(&self) -> Result<(), CliError> {
        let request = Request::Save {
            root: self.root.clone(),
        };
        match self.ask(&request)? {
            Answer::Saved => Ok(()),
            answer => Err(refused(answer)),
        }
    }

    fn ask(&self, request: &Request) -> Result<Answer, CliError> {
        dc_ipc::request(&self.channel, request).map_err(|error| match error {
            IpcError::NotRunning(_) => CliError::Failed(CLOSED.to_owned()),
            error => lost(error),
        })
    }
}

const CLOSED: &str = "редактор закрыл проект, пока шла команда: запусти её ещё раз";

/// An answer other than the one the request expects, as an error.
pub(crate) fn refused(answer: Answer) -> CliError {
    CliError::Failed(match answer {
        Answer::Refused(text) => text,
        Answer::NotOpen => CLOSED.to_owned(),
        other => format!(
            "редактор ответил не по делу ({other:?}): обнови DevCast, чтобы программа и devcast были одной версии"
        ),
    })
}

fn lost(error: IpcError) -> CliError {
    CliError::Failed(format!(
        "проект открыт в редакторе, но связаться с ним не удалось ({error}): закрой проект в редакторе или перезапусти DevCast"
    ))
}

impl Editor {
    /// `devcast save --as`: the run copied the state it read to `copy`; the editor drops its
    /// working state and opens the copy, as after «Сохранить как…».
    pub(crate) fn copied(&self, copy: PathBuf) -> Result<(), CliError> {
        let request = Request::Copied {
            root: self.root.clone(),
            revision: self.revision,
            copy,
        };
        match self.ask(&request)? {
            Answer::Opened => Ok(()),
            answer => Err(refused(answer)),
        }
    }
}

/// The channel of the running app: `channel` of the tests, else the real `devcast-app-<user>`.
pub(crate) fn app_channel(channel: Option<&str>) -> Result<String, CliError> {
    match channel {
        Some(channel) => Ok(channel.to_owned()),
        None => dc_ipc::socket_name(SERVICE).map_err(|error| {
            CliError::Failed(format!(
                "связаться с программой DevCast не удалось ({error}): перезапусти её и повтори команду"
            ))
        }),
    }
}

/// The channel of the app when `lock-owner.json` says it holds `dir` (a stale name is checked
/// by the app's answer).
pub(crate) fn held_by_app(
    dir: &ProjectDir,
    channel: Option<&str>,
) -> Result<Option<String>, CliError> {
    match LockOwner::of(dir).is_some_and(|owner| owner.app == LOCK_OWNER) {
        true => app_channel(channel).map(Some),
        false => Ok(None),
    }
}

/// Asks the app on `channel`; `None` when it is not running.
pub(crate) fn ask_app(channel: &str, request: &Request) -> Result<Option<Answer>, CliError> {
    match dc_ipc::request(channel, request) {
        Ok(answer) => Ok(Some(answer)),
        Err(IpcError::NotRunning(_)) => Ok(None),
        Err(error) => Err(lost(error)),
    }
}

/// The app is not running.
pub(crate) const NOT_RUNNING: &str = "DevCast не запущен: запусти программу и повтори команду";
