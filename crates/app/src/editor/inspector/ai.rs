//! The «Нейронка» tab (mockup 9): before a run — the recipes, «Ещё попросить», the agent and its
//! model, «Сделать красиво»; while it runs — «Ход работы» and «Остановить»; after it — its
//! words, «Отменить всё» and «Доделать или исправить». The choices are settings edits
//! (`Ai::edits`); what the tab asks of the run is an `Action::Ai`.

use std::time::{Duration, Instant};

use dc_settings::{Agent, AiEdit};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{self, Group, Header, header, padded, rows};
use dc_ui_kit::strings::ai as text;
use dc_ui_kit::tokens::{
    FS_M, FS_S, HUE_AI, HUE_AI_DONE, HUE_KEYS, ICON, OK, REC, S1, S2, TEXT, TEXT_2,
};
use dc_ui_kit::widgets::{Button, Checkbox, IconButton, Kind, dropdown, text_area, text_field};
use eframe::egui::{self, RichText, Ui};

use super::Cx;
use crate::editor::Action;
use crate::editor::ai::{Ai, AiAsk, Draft, Line, Phase};

/// Id of the tab in the remembered groups.
pub const ID: &str = "ai";

/// Rows of the request fields.
const ASK_ROWS: usize = 3;

/// The tab.
pub fn tab(ui: &mut Ui, cx: &mut Cx<'_>) {
    let (base, ai, actions) = (&mut cx.base, &mut *cx.ai, &mut cx.actions);
    if ai.agent().is_none() {
        not_found(ui, actions);
        return;
    }
    head(ui, ai);
    match ai.phase {
        Phase::Idle => {
            base.group(ui, group("recipes", text::RECIPES, true), |ui, _| {
                recipes(ui, ai)
            });
            base.group(ui, group("ask", text::ASK, true), |ui, _| {
                text_area(
                    ui,
                    &mut ai.ask,
                    text::ASK_HINT,
                    ui.available_width(),
                    ASK_ROWS,
                );
            });
            base.group(ui, group("agent", text::AGENT, false), |ui, _| {
                agent(ui, ai)
            });
            padded(ui, |ui| run_button(ui, ai, actions, text::RUN));
        }
        Phase::Running { .. } => {
            base.group(ui, group("log", text::LOG, true), |ui, _| log(ui, ai));
            padded(ui, |ui| {
                let stop = Button::new(text::STOP).icon(Icon::Stop).small();
                if ui.add(stop).clicked() {
                    actions.push(Action::Ai(AiAsk::Stop));
                }
                rows::note(ui, text::WHILE_RUNNING);
            });
        }
        _ => ended(ui, base, ai, actions),
    }
    draft(ui, ai);
}

/// The head: the run's colour and where it is.
fn head(ui: &mut Ui, ai: &Ai) {
    let now = Instant::now();
    let (hue, subtitle) = match &ai.phase {
        Phase::Idle => (HUE_AI, text::SUBTITLE.to_owned()),
        Phase::Running { .. } => {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
            (HUE_AI, text::running(ai.secs(now)))
        }
        Phase::Done { secs, .. } => (HUE_AI_DONE, text::done(*secs, ai.commands())),
        Phase::Stopped { secs } => (HUE_AI_DONE, text::stopped(*secs, ai.commands())),
        Phase::Failed { secs, .. } => (HUE_KEYS, text::failed(*secs)),
    };
    let head = Header {
        hue,
        title: text::TITLE,
        subtitle: &subtitle,
    };
    header(ui, &head, |_| {});
}

/// After a run: the log (closed), its words, «Отменить всё», «Доделать или исправить».
fn ended(ui: &mut Ui, base: &mut panel::Cx<'_>, ai: &mut Ai, actions: &mut Vec<Action>) {
    base.group(ui, group("log", text::LOG, false), |ui, _| log(ui, ai));
    padded(ui, |ui| {
        match &ai.phase {
            Phase::Done { said, .. } => said_line(ui, said),
            Phase::Failed { why, .. } => {
                ui.label(RichText::new(why).size(FS_S).color(REC));
            }
            _ => {}
        }
        ui.add_space(S1);
        let undo = Button::new(text::UNDO_ALL).icon(Icon::Undo).small();
        if ai.commands() > 0 && ui.add(undo).clicked() {
            actions.push(Action::Ai(AiAsk::UndoAll));
        }
    });
    base.group(ui, group("more", text::MORE, true), |ui, _| {
        text_area(
            ui,
            &mut ai.more,
            text::MORE_HINT,
            ui.available_width(),
            ASK_ROWS,
        );
        ui.horizontal(|ui| {
            run_button(ui, ai, actions, text::SEND);
            if ui.add(Button::new(text::NEW_RUN).small()).clicked() {
                actions.push(Action::Ai(AiAsk::Restart));
            }
        });
    });
}

fn group(id: &'static str, title: &'static str, open: bool) -> Group<'static> {
    Group {
        id,
        title,
        open,
        reset: None,
    }
}

/// The checked recipes; an own one can be edited and removed.
fn recipes(ui: &mut Ui, ai: &mut Ai) {
    for recipe in ai.choices.recipes() {
        let mut on = ai.choices.checked.contains(&recipe.id);
        ui.horizontal(|ui| {
            if ui.add(Checkbox::new(&mut on, &recipe.title)).changed() {
                let id = recipe.id.clone();
                ai.edits.push(AiEdit::Check { id, on });
            }
            if !recipe.own {
                return;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let small = |icon, label| IconButton::new(icon, label).size(24.0, ICON - 2.0);
                if ui.add(small(Icon::Trash, text::DELETE_RECIPE)).clicked() {
                    ai.edits.push(AiEdit::Remove(recipe.id.clone()));
                }
                if ui.add(small(Icon::Pen, text::EDIT_RECIPE)).clicked() {
                    ai.draft = Some(Draft {
                        old: Some(recipe.id.clone()),
                        name: recipe.title.clone(),
                        prompt: recipe.prompt.clone(),
                    });
                }
            });
        });
    }
    let own = Button::new(text::OWN_RECIPE)
        .icon(Icon::Plus)
        .kind(Kind::Ghost)
        .small();
    if ui.add(own).clicked() {
        ai.draft = Some(Draft::default());
    }
}

/// The agent when both are found, and the model.
fn agent(ui: &mut Ui, ai: &mut Ai) {
    let found: Vec<(Agent, &str)> = ai
        .found
        .iter()
        .map(|f| (f.agent, agent_name(f.agent)))
        .collect();
    if found.len() > 1 {
        let mut chosen = ai.agent().map_or(Agent::Claude, |f| f.agent);
        let picked = rows::row(ui, text::AGENT, |ui| {
            let width = ui.available_width();
            dropdown(ui, text::AGENT, &mut chosen, &found, width).changed()
        });
        if picked {
            ai.edits.push(AiEdit::Agent(Some(chosen)));
        }
    }
    let mut model = ai.choices.model.clone().unwrap_or_default();
    let typed = rows::row(ui, text::MODEL, |ui| {
        let width = ui.available_width();
        text_field(ui, &mut model, text::MODEL_HINT, width).lost_focus()
    });
    if typed && Some(model.as_str()) != ai.choices.model.as_deref() {
        ai.edits.push(AiEdit::Model(Some(model)));
    }
}

/// «Сделать красиво» / «Отправить»: grey while there is nothing to ask.
fn run_button(ui: &mut Ui, ai: &Ai, actions: &mut Vec<Action>, title: &str) {
    let ready = ai.request().is_some();
    let button = Button::new(title)
        .icon(Icon::Spark)
        .kind(Kind::Primary)
        .small();
    if ui.add_enabled(ready, button).clicked() {
        actions.push(Action::Ai(AiAsk::Run));
    }
    if !ready && ai.phase == Phase::Idle {
        rows::note(ui, text::NOTHING);
    }
}

/// «Ход работы»: a command a line (✓ done, ✕ failed, ● running) and the agent's words.
fn log(ui: &mut Ui, ai: &Ai) {
    for line in &ai.log {
        match line {
            Line::Command { line, ok } => {
                let (mark, color) = match ok {
                    Some(true) => ("✓", OK),
                    Some(false) => ("✕", REC),
                    None => ("●", dc_ui_kit::tokens::ACCENT_HI),
                };
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = S1;
                    ui.label(RichText::new(mark).monospace().size(FS_S).color(color));
                    let label =
                        egui::Label::new(RichText::new(line).monospace().size(FS_S).color(TEXT_2));
                    ui.add(label.wrap());
                });
            }
            Line::Said(said) => said_line(ui, said),
        }
    }
}

fn said_line(ui: &mut Ui, said: &str) {
    ui.add(egui::Label::new(RichText::new(said).size(FS_M).color(TEXT)).wrap());
}

/// «Нейронка не найдена»: what is needed, «Как поставить», «Проверить снова».
fn not_found(ui: &mut Ui, actions: &mut Vec<Action>) {
    let head = Header {
        hue: HUE_KEYS,
        title: text::TITLE,
        subtitle: text::NOT_FOUND,
    };
    header(ui, &head, |_| {});
    padded(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(text::NOT_FOUND_TEXT).size(FS_M).color(TEXT)).wrap());
        ui.add_space(S2);
        ui.horizontal(|ui| {
            if ui.add(Button::new(text::HOW_TO_INSTALL).small()).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(text::INSTALL_URL));
            }
            let again = Button::new(text::CHECK_AGAIN).kind(Kind::Ghost).small();
            if ui.add(again).clicked() {
                actions.push(Action::Ai(AiAsk::LookAgain));
            }
        });
    });
}

/// The «Свой рецепт» dialog while it is open.
fn draft(ui: &mut Ui, ai: &mut Ai) {
    let Some(draft) = ai.draft.as_mut() else {
        return;
    };
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("ai-recipe")).show(ui.ctx(), |ui| {
        ui.set_width(360.0);
        ui.label(
            RichText::new(text::OWN_RECIPE)
                .size(FS_M)
                .strong()
                .color(TEXT),
        );
        ui.add_space(S2);
        rows::row(ui, text::RECIPE_NAME, |ui| {
            let width = ui.available_width();
            text_field(ui, &mut draft.name, text::RECIPE_NAME_HINT, width);
        });
        text_area(
            ui,
            &mut draft.prompt,
            text::RECIPE_PROMPT,
            ui.available_width(),
            4,
        );
        ui.add_space(S2);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = Button::new(text::SAVE).kind(Kind::Primary).small();
            if ui.add(save).clicked() {
                answer = Some(true);
            }
            if ui
                .add(Button::new(text::CANCEL).kind(Kind::Ghost).small())
                .clicked()
            {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    match answer {
        Some(true) => {
            if let Some(Draft { old, name, prompt }) = ai.draft.take() {
                ai.edits.push(AiEdit::Put { old, name, prompt });
            }
        }
        Some(false) => ai.draft = None,
        None => {}
    }
}

fn agent_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => text::CLAUDE,
        Agent::Codex => text::CODEX,
    }
}
