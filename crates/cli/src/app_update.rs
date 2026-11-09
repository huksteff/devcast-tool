//! `devcast app update [--install]`: «Проверить обновления» of the running app's settings and,
//! with `--install`, its «Скачать» and «Перезапустить» (8.2). The app checks, downloads and
//! restarts itself; this run waits for its answer.

use dc_ops::live::{Answer as Live, Request, UpdateState};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::live::{NOT_RUNNING, app_channel, ask_app, refused};
use crate::run::Answer;

/// `devcast app update`.
pub(crate) fn update(ctx: &Ctx<'_>, install: bool) -> Result<Answer, CliError> {
    if ctx.globals.dry_run {
        let text = "dry-run: обновления не проверялись".to_owned();
        return Ok(Answer::new(text, json!({"dry_run": true})));
    }
    let request = Request::Update { install };
    match ask_app(&app_channel(ctx.local.app_channel)?, &request)? {
        Some(Live::Update(state)) => match &state {
            UpdateState::Failed { error } => Err(CliError::Failed(error.clone())),
            _ => Ok(Answer::new(line(&state, install), json!(state))),
        },
        Some(answer) => Err(refused(answer)),
        None => Err(CliError::Failed(NOT_RUNNING.to_owned())),
    }
}

/// The answer in words.
pub(crate) fn line(state: &UpdateState, install: bool) -> String {
    match state {
        UpdateState::NotInstalled => {
            "обновления работают только в установленной программе (DevCast-Setup.exe), а эта запущена из сборки".to_owned()
        }
        UpdateState::Unknown | UpdateState::Checking => "проверка ещё идёт: повтори команду".to_owned(),
        UpdateState::UpToDate { version } => format!("установлена последняя версия {version}"),
        UpdateState::Available { version } => match install {
            true => format!("доступна {version}, но не скачана: повтори команду"),
            false => format!("доступна версия {version}: devcast app update --install — скачать и перезапустить"),
        },
        UpdateState::Downloading { version, done } => {
            format!("скачивается {version}: {:.0}%", done * 100.0)
        }
        UpdateState::Ready { version } => match install {
            true => format!("{version} скачана: программа перезапускается в новую версию"),
            false => format!("{version} скачана и установится при выходе из программы"),
        },
        UpdateState::Failed { error } => error.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_reads_in_words() {
        let v = || "0.2.0".to_owned();
        let states = [
            UpdateState::NotInstalled,
            UpdateState::UpToDate { version: v() },
            UpdateState::Available { version: v() },
            UpdateState::Downloading {
                version: v(),
                done: 0.5,
            },
            UpdateState::Ready { version: v() },
        ];
        for state in states {
            assert!(!line(&state, false).is_empty());
        }
        assert!(line(&UpdateState::Available { version: v() }, false).contains("--install"));
        assert!(line(&UpdateState::Ready { version: v() }, true).contains("перезапускается"));
    }
}
