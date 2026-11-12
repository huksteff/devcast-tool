//! «Войти» of the «Нейронка» tab. Claude Code signs in without a console: `dc_agent::login`
//! tells the page, the tab opens it in the browser and hands over the code the page shows.
//! Codex uses the same piped login process; its browser page returns to its local callback.

use dc_agent::{Found, login::Login};
use dc_settings::AiChoices;
use dc_ui_kit::strings::ai as text;
use eframe::egui;

use super::ai::Ai;

impl Ai {
    /// A provider change starts its own conversation and discards any pending auth result.
    pub fn recall(&mut self, choices: AiChoices, ctx: &egui::Context) {
        let before = self.agent().map(|found| found.agent);
        self.choices = choices;
        if before != self.agent().map(|found| found.agent) {
            self.reset_agent();
            self.check_sign_in(ctx);
        }
    }

    /// Automatic selection can change providers when an installation appears or disappears.
    pub(super) fn discovered(&mut self, found: Vec<Found>, ctx: &egui::Context) {
        let before = self.agent().map(|found| found.agent);
        self.found = found;
        if before != self.agent().map(|found| found.agent) {
            self.reset_agent();
        }
        self.check_sign_in(ctx);
    }

    fn reset_agent(&mut self) {
        self.restart();
        self.cancel_login();
        self.signing = None;
        self.signed_in = None;
        self.refused = false;
        self.usage = None;
        self.limits = None;
    }

    /// «Войти».
    pub fn login(&mut self, ctx: &egui::Context) -> Result<(), String> {
        let agent = self
            .agent()
            .cloned()
            .ok_or_else(|| text::NOT_FOUND_TEXT.to_owned())?;
        let login = Login::start(&agent, self.waker(ctx)).map_err(|e| text::not_started(&e))?;
        self.code.clear();
        self.page = None;
        self.login = Some(login);
        Ok(())
    }

    /// The sign-in runs: the tab shows the field for the code.
    pub fn signing_in(&self) -> bool {
        self.login.is_some()
    }

    /// The page of the sign-in to open in the browser, once.
    pub fn take_page(&mut self) -> Option<String> {
        self.page.take()
    }

    /// «Отправить код».
    pub fn send_code(&mut self) -> Result<(), String> {
        let Some(login) = self.login.as_mut() else {
            return Ok(());
        };
        let sent = login.send_code(&self.code);
        self.code.clear();
        sent.map_err(|e| text::not_started(&e))
    }

    /// «Отмена»: the sign-in ends.
    pub fn cancel_login(&mut self) {
        self.login = None;
        self.page = None;
    }

    /// Every frame: the page once the agent told it; the end of the sign-in — the tab then
    /// looks again, asking the agent itself whether it works now.
    pub(super) fn sign_in_tick(&mut self) {
        let Some(login) = self.login.as_mut() else {
            return;
        };
        if let Some(page) = login.page() {
            self.page = Some(page);
        }
        if login.ended().is_some() {
            self.login = None;
            self.refused = true;
            self.looked = false;
        }
    }
}
