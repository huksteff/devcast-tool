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

use super::{Cx, ai_cards};
use crate::editor::Action;
use crate::editor::ai::{Ai, AiAsk, Draft, Line, Phase};

/// Id of the tab in the remembered groups.
pub const ID: &str = "ai";

/// Rows of the request fields.
const ASK_ROWS: usize = 3;

/// The tab.
pub fn tab(ui: &mut Ui, cx: &mut Cx<'_>) {
    let (base, ai, actions) = (&mut cx.base, &mut *cx.ai, &mut cx.actions);
    if !ai.looked {
        actions.push(Action::Ai(AiAsk::LookAgain));
        return;
    }
    if ai.agent().is_none() || ai.signed_in == Some(false) {
        if ai.found.len() > 1 || (!ai.found.is_empty() && ai.agent().is_none()) {
            base.group(ui, group("agent", text::AGENT, true), |ui, _| {
                agent(ui, ai, actions)
            });
        }
        match ai.agent() {
            None => ai_cards::not_found(ui, ai.choices.agent == Some(Agent::Codex), actions),
            Some(_) => ai_cards::signed_out(ui, ai, actions),
        }
        own(ui, base, ai, actions);
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
                agent(ui, ai, actions)
            });
            padded(ui, |ui| run_button(ui, ai, actions, text::RUN));
            own(ui, base, ai, actions);
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
        if let Some(line) = spent(ai) {
            rows::note(ui, &line);
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

/// «Opus 5.5 · Лимит 5 ч: 19 % → 21 % · неделя: 35 %» after a run: the model and how the
/// subscription's limits moved, as the agent told them.
fn spent(ai: &Ai) -> Option<String> {
    let model = ai
        .usage
        .as_ref()
        .and_then(|u| u.model.as_deref())
        .map(dc_agent::model_name);
    let limits = ai.limits.map(|(before, after)| {
        text::limits(
            (before.five_hour, after.five_hour),
            (before.seven_day, after.seven_day),
        )
    });
    let parts: Vec<String> = model
        .into_iter()
        .chain(limits)
        .filter(|p| !p.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The agent when both are found, and the model.
fn agent(ui: &mut Ui, ai: &mut Ai, actions: &mut Vec<Action>) {
    let found: Vec<(Agent, &str)> = ai
        .found
        .iter()
        .map(|f| (f.agent, agent_name(f.agent)))
        .collect();
    if found.len() > 1 || ai.agent().is_none() {
        let mut chosen = ai
            .agent()
            .map_or(ai.choices.agent.unwrap_or(Agent::Claude), |f| f.agent);
        let picked = rows::row(ui, text::AGENT, |ui| {
            let width = ui.available_width();
            dropdown(ui, text::AGENT, &mut chosen, &found, width).changed()
        });
        if picked {
            ai.edits.push(AiEdit::Agent(Some(chosen)));
        }
    }
    let codex = ai.agent().is_some_and(|f| f.agent == Agent::Codex);
    if codex && found.len() == 1 {
        rows::row(ui, text::AGENT, |ui| ui.label(text::CODEX));
    }
    model(ui, ai, codex);
    if codex && ai.models.loading() {
        rows::note(ui, text::MODELS_LOADING);
    }
    if codex && (ai.models.error.is_some() || (!ai.models.loading() && ai.models.items.is_empty()))
    {
        if let Some(error) = &ai.models.error {
            rows::note(ui, &text::models_failed(error));
        } else {
            rows::note(ui, text::MODELS_EMPTY);
        }
        if ui.add(Button::new(text::CHECK_AGAIN).small()).clicked() {
            actions.push(Action::Ai(AiAsk::RefreshModels));
        }
    }
}

fn model(ui: &mut Ui, ai: &mut Ai, codex: bool) {
    let chosen = ai.choices.model.clone();
    let mut models: Vec<(Option<String>, &str)> = vec![(None, text::MODEL_DEFAULT)];
    if ai.agent().is_some_and(|f| f.agent == Agent::Claude) {
        models.extend(CLAUDE_MODELS.map(|(id, title)| (Some(id.to_owned()), title)));
    }
    if codex {
        models.extend(
            ai.models
                .items
                .iter()
                .map(|m| (Some(m.model.clone()), m.display_name.as_str())),
        );
    }
    // A model set by `devcast recipes model` stays on the list.
    if let Some(own) = chosen
        .as_deref()
        .filter(|m| !models.iter().any(|(id, _)| id.as_deref() == Some(*m)))
    {
        models.push((Some(own.to_owned()), own));
    }
    let mut model = chosen.clone();
    let picked = rows::row(ui, text::MODEL, |ui| {
        let width = ui.available_width();
        dropdown(ui, text::MODEL, &mut model, &models, width).changed()
    });
    if picked && model != chosen {
        ai.edits.push(AiEdit::Model(model));
    }
}

/// The models of Claude Code by the aliases it takes (`--model sonnet`): the newest of each.
const CLAUDE_MODELS: [(&str, &str); 3] = [
    ("opus", text::OPUS),
    ("sonnet", text::SONNET),
    ("haiku", text::HAIKU),
];

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

/// «Своя нейронка», closed: the skill folder for agents opened without DevCast.
fn own(ui: &mut Ui, base: &mut panel::Cx<'_>, ai: &Ai, actions: &mut Vec<Action>) {
    if let Some(folder) = ai.skill.as_deref() {
        base.group(ui, group("own", text::OWN_AGENT, false), |ui, _| {
            ai_cards::own_agent(ui, folder, actions);
        });
    }
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
