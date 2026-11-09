//! Updates (8.2, decision 013): a check at the start and every 6 hours (`updates.auto_check`)
//! or by «Проверить обновления» and `devcast app update`, on a thread of its own; «Скачать?» is
//! a HUD popover (`hud/update.rs`), never while recording or exporting; the download goes on a
//! thread too and the update installs when the app exits («Перезапустить» exits at once).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dc_ops::live::{Answer, UpdateState};
use dc_update::{Update, UpdateError, Updater};
use eframe::egui;

use crate::app::Shared;
use crate::hud::popover::{self, Kind};
use crate::instance::Reply;

/// What the threads tell the app.
enum Event {
    Checked(Result<Option<Box<Update>>, UpdateError>),
    /// Downloaded share, 0..100 %.
    Progress(i16),
    Downloaded(Result<(), UpdateError>),
}

/// The updates of the app.
pub struct Updates {
    /// The updater of the channel `beta` says; made at the first tick and again when the
    /// setting changes. `None` inside: not an installation.
    updater: Option<(bool, Option<Arc<Updater>>)>,
    pub state: UpdateState,
    /// The version of `Available`, `Downloading` and `Ready`.
    found: Option<Update>,
    sender: Sender<Event>,
    events: Receiver<Event>,
    /// «Скачать?» waits until then, unix seconds; set whenever it is asked.
    snoozed_until: Option<u64>,
    /// «Готово» was told about the downloaded version.
    ready_told: bool,
    /// The update installs and the new version starts when the app exits.
    pub restart: bool,
    /// `devcast app update` runs waiting; `true` — with `--install`.
    waiters: Vec<(bool, Reply)>,
}

impl Default for Updates {
    fn default() -> Self {
        let (sender, events) = channel();
        Self {
            updater: None,
            state: UpdateState::Unknown,
            found: None,
            sender,
            events,
            snoozed_until: None,
            ready_told: false,
            restart: false,
            waiters: Vec::new(),
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Updates {
    fn busy(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        )
    }

    fn updater(&self) -> Option<Arc<Updater>> {
        self.updater.as_ref().and_then(|(_, u)| u.clone())
    }
}

impl Shared {
    /// Every frame of the main window: the updater of the channel, the threads' news, the
    /// automatic check and «Скачать?» when the HUD is free.
    pub fn updates_tick(&mut self) {
        let beta = self.store.get().updates.beta;
        if self
            .updates
            .updater
            .as_ref()
            .is_none_or(|(b, _)| *b != beta)
        {
            self.make_updater(beta);
        }
        while let Ok(event) = self.updates.events.try_recv() {
            self.updates_heard(event);
        }
        let settings = self.store.get().updates.clone();
        if settings.auto_check && self.updates.updater().is_some() && !self.updates.busy() {
            match dc_update::until_check(now(), settings.checked_at) {
                0 => self.check_updates(),
                wait => self
                    .ctx
                    .request_repaint_after_for(Duration::from_secs(wait), egui::ViewportId::ROOT),
            }
        }
        self.ask_about_update();
    }

    fn make_updater(&mut self, beta: bool) {
        let repository = dc_update::feedback().map(|f| f.repository.clone());
        let made = repository.and_then(|r| dc_update::Updater::installed(&r, beta));
        let updater = match made {
            Ok(updater) => updater.map(Arc::new),
            Err(error) => {
                self.updates.state = UpdateState::Failed {
                    error: error.to_string(),
                };
                None
            }
        };
        if updater.is_none() && !matches!(self.updates.state, UpdateState::Failed { .. }) {
            self.updates.state = UpdateState::NotInstalled;
        } else if updater.is_some() && !self.updates.busy() {
            self.updates.state = UpdateState::Unknown;
            self.updates.found = None;
        }
        self.updates.updater = Some((beta, updater));
    }

    /// «Проверить обновления»: on a thread; nothing while one goes on.
    pub fn check_updates(&mut self) {
        let Some(updater) = self.updates.updater() else {
            return self.answer_update_waiters();
        };
        if self.updates.busy() {
            return;
        }
        self.updates.state = UpdateState::Checking;
        let sender = self.updates.sender.clone();
        let ctx = self.ctx.clone();
        self.spawn_update_thread("devcast-update-check", move || {
            let _ = sender.send(Event::Checked(updater.check().map(|f| f.map(Box::new))));
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
    }

    /// «Скачать»: the found version, on a thread.
    pub fn download_update(&mut self) {
        let (Some(updater), Some(update)) = (self.updates.updater(), self.updates.found.clone())
        else {
            return;
        };
        if self.updates.busy() {
            return;
        }
        self.updates.state = UpdateState::Downloading {
            version: update.version.clone(),
            done: 0.0,
        };
        let sender = self.updates.sender.clone();
        let ctx = self.ctx.clone();
        self.spawn_update_thread("devcast-update-download", move || {
            let (progress, heard) = channel();
            let relay = sender.clone();
            let wake = ctx.clone();
            let relaying = std::thread::spawn(move || {
                for share in heard {
                    let _ = relay.send(Event::Progress(share));
                    wake.request_repaint_of(egui::ViewportId::ROOT);
                }
            });
            let downloaded = updater.download(&update, progress);
            let _ = relaying.join();
            let _ = sender.send(Event::Downloaded(downloaded));
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
    }

    fn spawn_update_thread(&mut self, name: &str, work: impl FnOnce() + Send + 'static) {
        if let Err(error) = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(work)
        {
            self.updates.state = UpdateState::Failed {
                error: format!("поток обновления не запустился: {error}"),
            };
        }
    }

    fn updates_heard(&mut self, event: Event) {
        let state = &mut self.updates.state;
        match event {
            Event::Checked(Ok(found)) => {
                *state = match &found {
                    Some(update) => UpdateState::Available {
                        version: update.version.clone(),
                    },
                    None => UpdateState::UpToDate {
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                    },
                };
                self.updates.found = found.map(|update| *update);
                let checked = self.store.update(|s| s.updates.checked_at = Some(now()));
                self.report(checked);
                self.after_check();
            }
            Event::Progress(share) => {
                if let UpdateState::Downloading { done, .. } = state {
                    *done = f32::from(share.clamp(0, 100)) / 100.0;
                }
            }
            Event::Downloaded(Ok(())) => {
                if let Some(update) = &self.updates.found {
                    *state = UpdateState::Ready {
                        version: update.version.clone(),
                    };
                }
                self.after_download();
            }
            Event::Checked(Err(error)) | Event::Downloaded(Err(error)) => {
                tracing::warn!("{error}");
                *state = UpdateState::Failed {
                    error: error.to_string(),
                };
                self.answer_update_waiters();
            }
        }
    }

    /// Asks «Скачать?» about a found version, or tells it is ready, when the HUD is shown and
    /// free: no recording, no export, no other popover, no editor in front.
    fn ask_about_update(&mut self) {
        let free = self.session.is_idle()
            && self.popover.is_none()
            && self.editor.is_none()
            && !self.exports.as_ref().is_some_and(|e| e.busy());
        if !free {
            return;
        }
        let skipped = self.store.get().updates.skipped.clone();
        let ask = match &self.updates.state {
            UpdateState::Available { version } => {
                let asked = dc_update::ask(
                    version,
                    skipped.as_deref(),
                    self.updates.snoozed_until,
                    now(),
                );
                if asked {
                    self.updates.snoozed_until = Some(now() + dc_update::SNOOZE);
                }
                asked
            }
            UpdateState::Ready { .. } => !std::mem::replace(&mut self.updates.ready_told, true),
            _ => false,
        };
        if ask {
            self.popover = Some(popover::Open::new(Kind::Update, 0.0));
        }
    }

    /// «Пропустить версию»: no question about this version again.
    pub fn skip_update(&mut self) {
        if let UpdateState::Available { version } = &self.updates.state {
            let version = version.clone();
            let skipped = self.store.update(|s| s.updates.skipped = Some(version));
            self.report(skipped);
        }
        self.popover = None;
    }

    /// «Перезапустить»: the app exits as «Выход» does and the new version starts.
    pub fn restart_into_update(&mut self) {
        if matches!(self.updates.state, UpdateState::Ready { .. }) {
            self.updates.restart = true;
            self.popover = None;
            self.quit();
        }
    }

    /// At the app's exit: a downloaded update installs now (decision 013).
    pub fn apply_update_at_exit(&mut self) {
        if !matches!(self.updates.state, UpdateState::Ready { .. }) {
            return;
        }
        if let (Some(updater), Some(update)) = (self.updates.updater(), &self.updates.found)
            && let Err(error) = updater.apply_at_exit(update, self.updates.restart)
        {
            tracing::error!("{error}");
        }
    }

    /// `devcast app update [--install]`: the answer once the check (and the download) ended.
    pub fn update_asked(&mut self, install: bool, reply: Reply) {
        if install && !self.updates_may_restart() {
            return reply.send(Answer::Refused(
                "идёт запись или экспорт: дождись конца и повтори devcast app update --install"
                    .to_owned(),
            ));
        }
        self.updates.waiters.push((install, reply));
        match self.updates.state {
            UpdateState::Ready { .. } if install => self.after_download(),
            _ => self.check_updates(),
        }
    }

    fn updates_may_restart(&self) -> bool {
        self.session.is_idle() && !self.exports.as_ref().is_some_and(|e| e.busy())
    }

    /// A check ended: the plain runs hear it; runs with `--install` wait for the download.
    fn after_check(&mut self) {
        let install = self.updates.waiters.iter().any(|(i, _)| *i);
        if install && matches!(self.updates.state, UpdateState::Available { .. }) {
            let plain = self.take_update_waiters(false);
            self.reply_update(plain);
            self.download_update();
        } else {
            self.answer_update_waiters();
        }
    }

    /// A download ended: runs with `--install` hear it and the app restarts into it.
    fn after_download(&mut self) {
        let install = self.take_update_waiters(true);
        let restart = !install.is_empty() && self.updates_may_restart();
        self.reply_update(install);
        if restart {
            self.updates.ready_told = true;
            self.restart_into_update();
        }
    }

    fn take_update_waiters(&mut self, install: bool) -> Vec<Reply> {
        let (taken, kept) = std::mem::take(&mut self.updates.waiters)
            .into_iter()
            .partition::<Vec<_>, _>(|(i, _)| *i == install);
        self.updates.waiters = kept;
        taken.into_iter().map(|(_, reply)| reply).collect()
    }

    fn answer_update_waiters(&mut self) {
        let all = std::mem::take(&mut self.updates.waiters);
        self.reply_update(all.into_iter().map(|(_, reply)| reply).collect());
    }

    fn reply_update(&self, replies: Vec<Reply>) {
        for reply in replies {
            reply.send(Answer::Update(self.updates.state.clone()));
        }
    }
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
