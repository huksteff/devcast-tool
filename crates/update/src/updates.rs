//! Updates from the GitHub Releases of the repository (8.2, decision 013): Velopack checks the
//! release feed of the channel, downloads the package next to the installation and applies it
//! when the app exits. CI packs betas on the channel `beta` and releases on Velopack's default
//! channel `win` (CONTRIBUTING.md, «Выпуск»).

use std::sync::mpsc::Sender;

use velopack::sources::GithubSource;
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, UpdateOptions};

use crate::error::UpdateError;

const STABLE: &str = "win";
const BETA: &str = "beta";

/// A newer version of the channel, found by [`Updater::check`].
#[derive(Debug, Clone)]
pub struct Update {
    /// SemVer of the release, `1.2.3` or `1.2.3-beta.4`.
    pub version: String,
    info: UpdateInfo,
}

/// The update manager of the installed app.
pub struct Updater {
    manager: UpdateManager,
}

impl Updater {
    /// The updater of this installation on the releases of `repository` (an `https://github.com/…`
    /// URL); with `beta`, the beta channel and pre-releases. `None` when the program does not
    /// run from a Velopack installation (a build of the repository).
    pub fn installed(repository: &str, beta: bool) -> Result<Option<Self>, UpdateError> {
        let source = GithubSource::new(repository, None, beta);
        let options = UpdateOptions {
            // Turning betas off keeps the beta until a newer release (betas are numbered after
            // the last release, CONTRIBUTING.md).
            AllowVersionDowngrade: false,
            ExplicitChannel: Some(channel(beta).to_owned()),
            MaximumDeltasBeforeFallback: 10,
        };
        match UpdateManager::new(source, Some(options), None) {
            Ok(manager) => Ok(Some(Self { manager })),
            Err(velopack::Error::NotInstalled(_)) => Ok(None),
            Err(error) => Err(UpdateError::Velopack(error.to_string())),
        }
    }

    /// The installed version.
    pub fn version(&self) -> String {
        self.manager.get_current_version_as_string()
    }

    /// The newest version of the channel when it is newer than the installed one.
    pub fn check(&self) -> Result<Option<Update>, UpdateError> {
        match self.manager.check_for_updates().map_err(network)? {
            UpdateCheck::UpdateAvailable(info) => Ok(Some(Update {
                version: info.TargetFullRelease.Version.clone(),
                info: *info,
            })),
            UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => Ok(None),
        }
    }

    /// Downloads `update`; `progress` hears 0..100 %.
    pub fn download(&self, update: &Update, progress: Sender<i16>) -> Result<(), UpdateError> {
        self.manager
            .download_updates(&update.info, Some(progress))
            .map_err(network)
    }

    /// Starts Velopack's updater, which waits for this process to exit, then installs the
    /// downloaded `update` and, with `restart`, starts the new version.
    pub fn apply_at_exit(&self, update: &Update, restart: bool) -> Result<(), UpdateError> {
        self.manager
            .wait_exit_then_apply_updates(&update.info, true, restart, Vec::<String>::new())
            .map_err(|e| UpdateError::Velopack(e.to_string()))
    }
}

fn channel(beta: bool) -> &'static str {
    if beta { BETA } else { STABLE }
}

fn network(error: velopack::Error) -> UpdateError {
    UpdateError::Network(error.to_string())
}

/// Seconds between automatic checks.
pub const CHECK_EVERY: u64 = 6 * 3600;
/// Seconds «Не сейчас» puts the question off.
pub const SNOOZE: u64 = 3 * 3600;

/// Seconds before a failed check is tried again: a computer without a network must not ask
/// GitHub in a loop (its limit is 60 requests an hour for the whole office).
pub const RETRY_AFTER: u64 = 15 * 60;

/// Seconds from `now` until the next automatic check (0: now): the last good one ended at
/// `checked_at`, the last failed one at `failed_at`; unix seconds. A clock set back checks
/// at once.
pub fn until_check(now: u64, checked_at: Option<u64>, failed_at: Option<u64>) -> u64 {
    let after = |at: Option<u64>, wait: u64| match at {
        Some(at) if at <= now => (at + wait).saturating_sub(now),
        _ => 0,
    };
    after(checked_at, CHECK_EVERY).max(after(failed_at, RETRY_AFTER))
}

/// Whether the app asks «Скачать?» about `version` at `now`: not a version the user skipped,
/// not before «Не сейчас» runs out at `snoozed_until`.
pub fn ask(version: &str, skipped: Option<&str>, snoozed_until: Option<u64>, now: u64) -> bool {
    skipped != Some(version) && snoozed_until.is_none_or(|until| now >= until)
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
