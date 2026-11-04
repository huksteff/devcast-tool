//! The cards of the «Нейронка» tab besides the run: no agent found, the agent not signed in
//! («Войти» opens its sign-in), and «Своя нейронка» — the skill folder for agents opened
//! without DevCast (8.10) with the request to hand them.

use std::path::Path;

use dc_ui_kit::panel::{Header, header, padded, rows};
use dc_ui_kit::strings::ai as text;
use dc_ui_kit::tokens::{FS_M, FS_S, HUE_KEYS, S2, TEXT, TEXT_2};
use dc_ui_kit::widgets::{Button, Kind};
use eframe::egui::{self, RichText, Ui};

use crate::editor::Action;
use crate::editor::ai::AiAsk;

/// «Нейронка не найдена»: what is needed, «Как поставить», «Проверить снова».
pub(super) fn not_found(ui: &mut Ui, actions: &mut Vec<Action>) {
    card(ui, text::NOT_FOUND, text::NOT_FOUND_TEXT, |ui| {
        if ui.add(Button::new(text::HOW_TO_INSTALL).small()).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(text::INSTALL_URL));
        }
        check_again(ui, actions);
    });
}

/// The agent is not signed in: «Войти» opens its sign-in, then «Проверить снова».
pub(super) fn signed_out(ui: &mut Ui, actions: &mut Vec<Action>) {
    card(ui, text::SIGNED_OUT, text::SIGNED_OUT_TEXT, |ui| {
        let sign_in = Button::new(text::SIGN_IN).kind(Kind::Primary).small();
        if ui.add(sign_in).clicked() {
            actions.push(Action::Ai(AiAsk::Login));
        }
        check_again(ui, actions);
    });
}

/// «Своя нейронка»: the skill folder and the request to copy.
pub(super) fn own_agent(ui: &mut Ui, folder: &Path, actions: &mut Vec<Action>) {
    rows::note(ui, text::OWN_AGENT_TEXT);
    let path = folder.display().to_string();
    ui.add(egui::Label::new(RichText::new(&path).monospace().size(FS_S).color(TEXT_2)).wrap());
    ui.add_space(S2);
    ui.horizontal(|ui| {
        if ui.add(Button::new(text::COPY_ASK).small()).clicked() {
            ui.ctx().copy_text(dc_agent::skills::ask(folder));
        }
        let open = Button::new(text::OPEN_FOLDER).kind(Kind::Ghost).small();
        if ui.add(open).clicked() {
            actions.push(Action::Ai(AiAsk::OpenSkill));
        }
    });
}

fn check_again(ui: &mut Ui, actions: &mut Vec<Action>) {
    let again = Button::new(text::CHECK_AGAIN).kind(Kind::Ghost).small();
    if ui.add(again).clicked() {
        actions.push(Action::Ai(AiAsk::LookAgain));
    }
}

/// A grey head «Сделать красиво · <why>», the text and the buttons.
fn card(ui: &mut Ui, why: &str, said: &str, buttons: impl FnOnce(&mut Ui)) {
    let head = Header {
        hue: HUE_KEYS,
        title: text::TITLE,
        subtitle: why,
    };
    header(ui, &head, |_| {});
    padded(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(said).size(FS_M).color(TEXT)).wrap());
        ui.add_space(S2);
        ui.horizontal(buttons);
    });
}
