//! The «Нейронка» tab (8.11, mockup 9) with a pretend agent: the recipes and the request,
//! a run to its end with its log, «Отменить всё» of the run's undo step, the own recipe dialog,
//! no agent found.

use std::path::Path;
use std::time::Instant;

use dc_agent::Found;
use dc_core::History;
use dc_ops::Registry;
use dc_project::history_log::Event;
use dc_settings::{Agent, AiEdit};
use dc_ui_kit::strings::ai as text;
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{H, project_json, step};
use super::zoom_tests::tall;
use crate::editor::ai::{Ai, Line, Phase};
use crate::editor::editing_tests::json;

#[path = "ai_login_tests.rs"]
mod login;
#[path = "ai_model_tests.rs"]
mod models;

/// A pretend Claude Code: a batch file that ignores its arguments and writes `answer`.
fn pretend(dir: &Path, answer: &str) -> Found {
    // cmd reads a batch file in the OEM code page: the path it types stays ASCII.
    let lines = dir.join("answer.jsonl");
    std::fs::write(&lines, answer).unwrap();
    let script = dir.join("agent.cmd");
    std::fs::write(&script, format!("@type \"{}\"\r\n", lines.display())).unwrap();
    Found {
        agent: Agent::Claude,
        path: script,
    }
}

const ANSWER: &str = concat!(
    r#"{"type":"system","subtype":"init","session_id":"s-1"}"#,
    "\n",
    r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Смотрю паузы."},{"type":"tool_use","name":"Bash","input":{"command":"devcast silences"}}]}}"#,
    "\n",
    r#"{"type":"user","message":{"content":[{"type":"tool_result","is_error":false}]}}"#,
    "\n",
    r#"{"type":"result","is_error":false,"result":"Вырезал 14 пауз.","session_id":"s-1"}"#,
    "\n"
);

fn ai_tab(found: Vec<Found>) -> H {
    let mut harness = tall();
    let editor = &mut harness.state_mut().editor;
    editor.ai = Ai::new(found);
    editor.view.tab = super::AI.id;
    harness.run();
    harness
}

fn click(harness: &mut H, role: egui::Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.step();
    // The running agent deliberately repaints; only idle UI is expected to settle.
    if !harness.state().editor.ai.running() {
        harness.run();
    }
}

/// Focuses the field and types `typed` into it.
fn type_in(harness: &mut H, label: &str, typed: &str) {
    harness.get_by_label(label).click();
    harness.run();
    harness.get_by_label(label).type_text(typed);
    harness.run();
}

#[test]
fn the_recipes_and_the_request_are_the_choices_of_the_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let mut harness = ai_tab(vec![pretend(tmp.path(), ANSWER)]);
    harness.snapshot("inspector_ai");
    click(&mut harness, egui::Role::CheckBox, "Убрать оговорки и «ээ»");
    click(&mut harness, egui::Role::CheckBox, "Убрать паузы");
    let edits = std::mem::take(&mut harness.state_mut().editor.ai.edits);
    let check = |id: &str, on| AiEdit::Check {
        id: id.to_owned(),
        on,
    };
    assert_eq!(edits, vec![check("slips", true), check("pauses", false)]);
    harness.state_mut().editor.ai.choices.checked.clear();
    harness.run();
    let run = harness.get_by_role_and_label(egui::Role::Button, text::RUN);
    assert!(run.accesskit_node().is_disabled(), "nothing to ask");
    assert!(harness.query_by_label(text::NOTHING).is_some());
}

/// The long caption of a model stays in its drop-down: the inspector keeps its width (it
/// narrowed, the preview covered the tab rail and the other tabs could not be opened).
#[test]
fn a_long_model_keeps_the_inspector_width() {
    let tmp = tempfile::tempdir().unwrap();
    let mut harness = ai_tab(vec![pretend(tmp.path(), ANSWER)]);
    // The right edge of the preview's toolbar is the left edge of the inspector.
    let preview_edge = |harness: &H| {
        harness
            .get_by_role_and_label(
                egui::Role::RadioButton,
                dc_ui_kit::strings::editor::QUALITY_FAST,
            )
            .accesskit_node()
            .bounding_box()
    };
    let before = preview_edge(&harness);
    // The group «Нейронка» with the model's drop-down, opened (its title is also the tab's).
    let state = harness.state_mut();
    state.groups.0.insert("ai.agent".to_owned(), true);
    state.editor.ai.choices.model = Some("sonnet".to_owned());
    harness.run();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(egui::Role::ComboBox, text::MODEL)
            .is_some(),
        "the group is open"
    );
    assert_eq!(preview_edge(&harness), before, "the preview did not grow");
    harness.snapshot("inspector_ai_long_model");
}

#[cfg(windows)]
#[test]
fn a_run_shows_its_log_and_ends_with_the_agents_words() {
    let tmp = tempfile::tempdir().unwrap();
    let mut harness = ai_tab(vec![pretend(tmp.path(), ANSWER)]);
    click(&mut harness, egui::Role::Button, text::RUN);
    assert!(
        matches!(harness.state().editor.ai.phase, Phase::Running { .. }),
        "{:?}",
        harness.state().outcome.as_ref().map(|o| &o.errors)
    );
    harness.state_mut().editor.ai.settle();
    harness.run();
    let ai = &harness.state().editor.ai;
    let Phase::Done { said, .. } = &ai.phase else {
        panic!("{:?}", ai.phase);
    };
    assert_eq!(said, "Вырезал 14 пауз.");
    let command = Line::Command {
        line: "devcast silences".to_owned(),
        ok: Some(true),
    };
    assert_eq!(
        ai.log,
        vec![Line::Said("Смотрю паузы.".to_owned()), command]
    );
    assert!(harness.query_by_label(text::MORE).is_some());
    let send = harness.get_by_role_and_label(egui::Role::Button, text::SEND);
    assert!(send.accesskit_node().is_disabled(), "nothing to add yet");
    click(&mut harness, egui::Role::Button, text::NEW_RUN);
    assert_eq!(harness.state().editor.ai.phase, Phase::Idle);
}

#[test]
fn undo_all_takes_back_the_runs_step_only() {
    let mut harness = ai_tab(vec![Found {
        agent: Agent::Claude,
        path: "claude.exe".into(),
    }]);
    let before = project_json(&harness);
    // What the agent's two `devcast` commands sent this editor: one step of group «ai-1».
    let editor = &mut harness.state_mut().editor;
    let mut project = editor.doc.project().clone();
    let mut history = History::default();
    history.group(Some("ai-1".to_owned()));
    let registry = Registry::devcast().unwrap();
    for line in ["keys --show", "keys --size 1.5"] {
        dc_ops::apply(&registry, &mut project, &mut history, &[step(line)], false).unwrap();
    }
    let change = history.changes()[0].clone();
    let revision = editor.doc.revision();
    editor
        .doc
        .take(revision, vec![Event::Do(change)], Instant::now())
        .unwrap();
    editor.ai.group = Some("ai-1".to_owned());
    editor.ai.phase = Phase::Done {
        secs: 3,
        said: "Включил клавиши.".to_owned(),
    };
    editor.ai.log.push(Line::Command {
        line: "devcast keys --show".to_owned(),
        ok: Some(true),
    });
    harness.run();
    assert_eq!(project_json(&harness), json(&project));
    harness.snapshot("inspector_ai_done");
    click(&mut harness, egui::Role::Button, text::UNDO_ALL);
    assert_eq!(project_json(&harness), before, "both commands go back");
    click(&mut harness, egui::Role::Button, text::UNDO_ALL);
    assert_eq!(
        project_json(&harness),
        before,
        "an older step is not the run's"
    );
}

#[test]
fn an_own_recipe_is_written_in_its_dialog() {
    let mut harness = ai_tab(vec![Found {
        agent: Agent::Claude,
        path: "claude.exe".into(),
    }]);
    click(&mut harness, egui::Role::Button, text::OWN_RECIPE);
    type_in(&mut harness, text::RECIPE_NAME_HINT, "Лого");
    type_in(&mut harness, text::RECIPE_PROMPT, "Вставь лого в угол");
    harness.snapshot("inspector_ai_recipe");
    click(&mut harness, egui::Role::Button, text::SAVE);
    let put = AiEdit::Put {
        old: None,
        name: "Лого".to_owned(),
        prompt: "Вставь лого в угол".to_owned(),
    };
    assert_eq!(harness.state().editor.ai.edits, vec![put]);
    assert!(harness.state().editor.ai.draft.is_none());
}

#[test]
fn without_an_agent_the_tab_says_what_is_needed() {
    let mut harness = ai_tab(Vec::new());
    assert!(harness.query_by_label(text::NOT_FOUND_TEXT).is_some());
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Button, text::RUN)
            .is_none()
    );
    harness.snapshot("inspector_ai_not_found");
}

#[test]
fn a_signed_out_agent_offers_its_sign_in_and_the_skill_for_own_agents() {
    let mut harness = ai_tab(vec![Found {
        agent: Agent::Claude,
        path: "нет такой программы.exe".into(),
    }]);
    let folder = std::path::PathBuf::from(r"C:\Users\Кто-то\AppData\Roaming\DevCast\agent-skill");
    let ai = &mut harness.state_mut().editor.ai;
    ai.signed_in = Some(false);
    ai.skill = Some(folder.clone());
    harness.run();
    assert!(harness.query_by_label(text::SIGNED_OUT_TEXT).is_some());
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Button, text::RUN)
            .is_none()
    );
    harness.get_by_label(text::OWN_AGENT).click();
    harness.run();
    assert!(
        harness
            .query_by_label(&folder.display().to_string())
            .is_some()
    );
    harness.snapshot("inspector_ai_signed_out");
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Button, text::SIGN_IN)
            .is_some()
    );
}

#[test]
fn a_signed_out_claude_does_not_hide_the_codex_selector() {
    let mut harness = ai_tab(vec![
        Found {
            agent: Agent::Claude,
            path: "missing-claude.exe".into(),
        },
        Found {
            agent: Agent::Codex,
            path: "missing-codex.exe".into(),
        },
    ]);
    harness.state_mut().editor.ai.signed_in = Some(false);
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(egui::Role::ComboBox, text::AGENT)
            .is_some()
    );
}

#[test]
fn a_missing_codex_can_be_replaced_with_the_only_installed_agent() {
    let mut harness = ai_tab(vec![Found {
        agent: Agent::Claude,
        path: "missing-claude.exe".into(),
    }]);
    harness.state_mut().editor.ai.choices.agent = Some(Agent::Codex);
    harness.run();
    click(&mut harness, egui::Role::ComboBox, text::AGENT);
    harness.get_by_label(text::CLAUDE).click();
    harness.run();
    assert!(
        harness
            .state()
            .editor
            .ai
            .edits
            .contains(&AiEdit::Agent(Some(Agent::Claude)))
    );
}

#[cfg(windows)]
#[test]
fn changing_to_codex_starts_a_fresh_conversation_and_clears_claude_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let mut harness = ai_tab(vec![
        pretend(tmp.path(), ANSWER),
        Found {
            agent: Agent::Codex,
            path: "missing-codex.exe".into(),
        },
    ]);
    click(&mut harness, egui::Role::Button, text::RUN);
    harness.state_mut().editor.ai.settle();
    let ai = &mut harness.state_mut().editor.ai;
    ai.more = "Добавь зумы".into();
    assert!(
        ai.request().unwrap().1,
        "Claude has a conversation to resume"
    );
    ai.signed_in = Some(false);
    let mut choices = ai.choices.clone();
    choices.edit(AiEdit::Agent(Some(Agent::Codex))).unwrap();
    ai.recall(choices, &egui::Context::default());
    assert_eq!(ai.phase, Phase::Idle);
    assert_eq!(ai.signed_in, None);
    assert!(
        !ai.request().unwrap().1,
        "Codex must not resume a Claude session"
    );
    harness.run();
}

#[cfg(windows)]
#[test]
fn automatic_selection_does_not_resume_a_disappeared_providers_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mut harness = ai_tab(vec![pretend(tmp.path(), ANSWER)]);
    click(&mut harness, egui::Role::Button, text::RUN);
    let ai = &mut harness.state_mut().editor.ai;
    ai.settle();
    ai.more = "Добавь зумы".into();
    assert!(ai.request().unwrap().1);
    ai.discovered(
        vec![Found {
            agent: Agent::Codex,
            path: "missing-codex.exe".into(),
        }],
        &egui::Context::default(),
    );
    assert_eq!(ai.phase, Phase::Idle);
    assert!(!ai.request().unwrap().1);
}
