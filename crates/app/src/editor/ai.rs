//! «Сделать красиво» of the editor (8.11, mockup 9): the run of the user's AI agent
//! (`dc-agent`) on the open project. Its `devcast` commands reach this editor live (7.32) and
//! join one undo step by their group (`DEVCAST_GROUP`, `History::group`). The UI never waits:
//! the agent's reader thread wakes the main window, the editor takes the events in its frame.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use dc_agent::{Ask, Event, Found, Job, Places, find, launch, pick, prompt};
use dc_settings::{AiChoices, AiEdit};
use dc_ui_kit::strings::ai as text;
use eframe::egui;

/// What the tab asks of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAsk {
    /// «Сделать красиво» or «Отправить».
    Run,
    Stop,
    /// «Отменить всё»: the run's undo step, while it is on top.
    UndoAll,
    /// «Начать заново».
    Restart,
    /// «Проверить снова».
    LookAgain,
    /// «Войти»: the agent's sign-in; its page opens in the browser.
    Login,
    /// «Отправить код» of the sign-in.
    SendCode,
    /// «Отмена» of the sign-in.
    CancelLogin,
    /// «Открыть папку» of «Своя нейронка».
    OpenSkill,
}

/// A line of «Ход работы».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A command the agent ran; `ok` once it ended.
    Command { line: String, ok: Option<bool> },
    /// The agent's words.
    Said(String),
}

/// Where the run is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running {
        since: Instant,
    },
    /// It ended with its last words.
    Done {
        secs: u64,
        said: String,
    },
    Stopped {
        secs: u64,
    },
    Failed {
        secs: u64,
        why: String,
    },
}

/// The own recipe being written: `old` — the name of the one it replaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub old: Option<String>,
    pub name: String,
    pub prompt: String,
}

#[derive(Debug)]
pub struct Ai {
    /// The agents on this computer, looked for when the editor opens and on «Проверить снова».
    pub found: Vec<Found>,
    /// The choices of the settings, taken each frame.
    pub choices: AiChoices,
    /// «Ещё попросить» and «Доделать или исправить».
    pub ask: String,
    pub more: String,
    pub log: Vec<Line>,
    pub phase: Phase,
    /// The «Свой рецепт» dialog while it is open.
    pub draft: Option<Draft>,
    /// Settings edits of the tab; the frame writes them to the settings.
    pub edits: Vec<AiEdit>,
    /// The undo step of the last run: «Отменить всё» undoes it while it is on top.
    pub group: Option<String>,
    /// What the last run cost, as the agent counted it.
    pub usage: Option<dc_agent::Usage>,
    /// How full the subscription's limits were at the first and the last word of the run about
    /// them.
    pub limits: Option<(dc_agent::Limits, dc_agent::Limits)>,
    /// Signed in to the user's account: none until checked, or when the agent cannot tell.
    pub signed_in: Option<bool>,
    /// A run failed because the sign-in was refused: «Проверить снова» asks the agent itself,
    /// not its status.
    pub(super) refused: bool,
    /// The agents were looked for.
    pub looked: bool,
    /// The folder with the skill for agents opened without DevCast (8.10).
    pub skill: Option<PathBuf>,
    signing: Option<Receiver<Option<bool>>>,
    /// The sign-in that runs («Войти»), the code typed for it and its page until the tab
    /// opens it (`ai_sign_in.rs`).
    pub(super) login: Option<dc_agent::login::Login>,
    pub code: String,
    pub(super) page: Option<String>,
    session: Option<String>,
    job: Option<Job>,
    news: Arc<AtomicBool>,
}

impl Default for Ai {
    /// Not looked for yet: the first look at the tab looks (`look_again`), so an editor whose
    /// tab stays closed starts no process.
    fn default() -> Self {
        Self {
            looked: false,
            ..Self::new(Vec::new())
        }
    }
}

impl Ai {
    pub fn new(found: Vec<Found>) -> Self {
        Self {
            found,
            choices: AiChoices::default(),
            ask: String::new(),
            more: String::new(),
            log: Vec::new(),
            phase: Phase::Idle,
            draft: None,
            edits: Vec::new(),
            group: None,
            usage: None,
            limits: None,
            signed_in: None,
            refused: false,
            looked: true,
            skill: None,
            signing: None,
            login: None,
            code: String::new(),
            page: None,
            session: None,
            job: None,
            news: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The agent the next run calls.
    pub fn agent(&self) -> Option<&Found> {
        pick(&self.found, self.choices.agent)
    }

    pub fn running(&self) -> bool {
        matches!(self.phase, Phase::Running { .. })
    }

    /// The commands run so far.
    pub fn commands(&self) -> usize {
        self.log
            .iter()
            .filter(|l| matches!(l, Line::Command { .. }))
            .count()
    }

    /// The request of the next run: a new conversation with the checked recipes and «Ещё
    /// попросить», or, after a run, «Доделать или исправить» in the same conversation.
    pub fn request(&self) -> Option<(String, bool)> {
        match &self.session {
            Some(_) if self.phase != Phase::Idle => prompt::more(&self.more).map(|r| (r, true)),
            _ => prompt::first(&self.choices.checked_recipes(), &self.ask).map(|r| (r, false)),
        }
    }

    /// Starts the agent on the project in `project`.
    pub fn start(&mut self, ctx: &egui::Context, project: &Path) -> Result<(), String> {
        if self.running() {
            return Ok(());
        }
        let (request, more) = self.request().ok_or_else(|| text::NOTHING.to_owned())?;
        let agent = self
            .agent()
            .cloned()
            .ok_or_else(|| text::NOT_FOUND_TEXT.to_owned())?;
        let not_started = |e: &dyn std::fmt::Display| text::not_started(e);
        let system = dc_agent::system_file().map_err(|e| not_started(&e))?;
        let exe = std::env::current_exe().map_err(|e| not_started(&e))?;
        let devcast = exe.parent().unwrap_or(&exe).to_path_buf();
        let group = new_group();
        let ask = Ask {
            found: &agent,
            model: self.choices.model.as_deref(),
            project,
            devcast: &devcast,
            system: &system,
            request: &request,
            resume: if more { self.session.as_deref() } else { None },
            group: &group,
        };
        let job = Job::start(launch(&ask), self.waker(ctx)).map_err(|e| not_started(&e))?;
        if more {
            self.more.clear();
        } else {
            self.log.clear();
        }
        self.job = Some(job);
        self.group = Some(group);
        self.usage = None;
        self.limits = None;
        self.phase = Phase::Running {
            since: Instant::now(),
        };
        Ok(())
    }

    /// Takes the agent's events; true when the tab has news to draw.
    pub fn receive(&mut self, now: Instant) -> bool {
        self.sign_in_tick();
        let signed = self.signing.as_ref().and_then(|s| s.try_recv().ok());
        if let Some(signed) = signed {
            self.signed_in = signed;
            self.signing = None;
            self.refused &= signed != Some(true);
        }
        let Some(job) = &self.job else {
            return signed.is_some();
        };
        let events = job.receive();
        let news = !events.is_empty();
        for event in events {
            self.take(event, now);
        }
        news
    }

    fn take(&mut self, event: Event, now: Instant) {
        match event {
            Event::Session(id) => self.session = Some(id),
            Event::Command(line) => self.log.push(Line::Command { line, ok: None }),
            Event::Done { ok } => {
                let open = self.log.iter_mut().rev().find_map(|l| match l {
                    Line::Command {
                        ok: state @ None, ..
                    } => Some(state),
                    _ => None,
                });
                if let Some(state) = open {
                    *state = Some(ok);
                }
            }
            Event::Said(said) => self.log.push(Line::Said(said)),
            Event::Usage(usage) => self.usage = Some(usage),
            Event::Limits(now) => {
                let first = self.limits.map_or(now, |(first, _)| first);
                self.limits = Some((first, now));
            }
            Event::Finished(end) => {
                let secs = self.secs(now);
                self.job = None;
                self.phase = match end {
                    Ok(said) => Phase::Done { secs, said },
                    // The sign-in is gone: the tab asks to sign in instead of an error to read.
                    Err(why) if dc_agent::sign_in::refused(&why) => {
                        self.refused = true;
                        self.signed_in = Some(false);
                        Phase::Idle
                    }
                    Err(why) => Phase::Failed { secs, why },
                };
            }
        }
    }

    /// «Остановить»: the agent and its commands end; what it did stays, one undo step.
    pub fn stop(&mut self, now: Instant) {
        if let Some(mut job) = self.job.take() {
            job.stop();
            self.phase = Phase::Stopped {
                secs: self.secs(now),
            };
        }
    }

    /// «Начать заново»: a new conversation.
    pub fn restart(&mut self) {
        self.stop(Instant::now());
        self.phase = Phase::Idle;
        self.session = None;
        self.log.clear();
        self.more.clear();
    }

    /// The first look and «Проверить снова»: the agents, whether one is signed in, and the
    /// skill for agents opened without DevCast.
    pub fn look_again(&mut self, ctx: &egui::Context) -> Result<(), String> {
        self.looked = true;
        self.found = find(&Places::here());
        self.check_sign_in(ctx);
        self.write_skill()
    }

    /// Asks the agent whether it is signed in, on a thread that wakes the main window: the
    /// answer is a process, too slow for a frame.
    pub fn check_sign_in(&mut self, ctx: &egui::Context) {
        let Some(agent) = self.agent().cloned() else {
            self.signed_in = None;
            return;
        };
        let (send, answer) = channel();
        let wake = self.waker(ctx);
        // After a refused run the status is not believed: the agent is asked for a word.
        let probe = self.refused;
        let started = std::thread::Builder::new()
            .name("devcast-agent-auth".to_owned())
            .spawn(move || {
                let signed = match probe {
                    true => dc_agent::sign_in::answers(&agent),
                    false => dc_agent::sign_in::signed_in(&agent),
                };
                if send.send(signed).is_ok() {
                    wake();
                }
            });
        if started.is_ok() {
            self.signing = Some(answer);
        }
    }

    /// Writes the skill for agents opened without DevCast into the app settings folder.
    pub fn write_skill(&mut self) -> Result<(), String> {
        let folder = dc_project::settings::dir()
            .map_err(|e| e.to_string())?
            .join(dc_agent::skills::FOLDER);
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let devcast = exe.parent().unwrap_or(&exe);
        dc_agent::skills::write(&folder, devcast).map_err(|e| text::skill_not_written(&e))?;
        self.skill = Some(folder);
        Ok(())
    }

    /// Seconds since the start of the run.
    pub fn secs(&self, now: Instant) -> u64 {
        match self.phase {
            Phase::Running { since } => now.saturating_duration_since(since).as_secs(),
            Phase::Done { secs, .. } | Phase::Stopped { secs } | Phase::Failed { secs, .. } => secs,
            Phase::Idle => 0,
        }
    }

    /// The agent said something since the last call: the editor repaints.
    pub fn take_news(&self) -> bool {
        self.news.swap(false, Ordering::AcqRel)
    }

    pub(super) fn waker(&self, ctx: &egui::Context) -> impl Fn() + Send + Clone + 'static {
        let (ctx, news) = (ctx.clone(), Arc::clone(&self.news));
        move || {
            news.store(true, Ordering::Release);
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }

    /// Waits for the run to end (tests): every event is taken.
    #[cfg(test)]
    pub fn settle(&mut self) {
        while self.running() {
            let Some(job) = &self.job else { return };
            match job.next() {
                Some(event) => self.take(event, Instant::now()),
                None => return,
            }
        }
    }
}

/// An id of one run's undo step.
fn new_group() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("ai-{nanos}")
}
