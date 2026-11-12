//! The Codex model catalog stays off the UI thread and is discarded on provider changes.

use dc_agent::models::{Catalog, Model};
use dc_settings::Agent;
use eframe::egui;

use super::ai::Ai;

#[derive(Debug, Default)]
pub struct Models {
    pub items: Vec<Model>,
    pub error: Option<String>,
    pending: Option<Catalog>,
}

impl Models {
    pub fn loading(&self) -> bool {
        self.pending.is_some()
    }

    pub fn receive(&mut self) -> bool {
        let Some(answer) = self.pending.as_ref().and_then(Catalog::poll) else {
            return false;
        };
        self.pending = None;
        match answer {
            Ok(items) => self.items = items,
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }
}

impl Ai {
    pub(super) fn refresh_models(&mut self, ctx: &egui::Context) {
        self.models = Models::default();
        let Some(found) = self.agent().filter(|f| f.agent == Agent::Codex) else {
            return;
        };
        match Catalog::start(found, self.waker(ctx)) {
            Ok(pending) => self.models.pending = Some(pending),
            Err(error) => self.models.error = Some(error.to_string()),
        }
    }
}
