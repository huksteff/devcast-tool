//! Model selection uses the common picker and preserves settings edits.

use super::*;

#[test]
fn codex_has_its_own_model_popup_and_login_instructions() {
    let mut harness = ai_tab(vec![Found {
        agent: Agent::Codex,
        path: "missing-codex.exe".into(),
    }]);
    harness.state_mut().editor.ai.models.items = serde_json::from_value(serde_json::json!([
        {"model":"codex-fast","displayName":"Codex Fast"},
        {"model":"codex-strong","displayName":"Codex Strong"}
    ]))
    .unwrap();
    harness.state_mut().groups.0.insert("ai.agent".into(), true);
    harness.run();
    click(&mut harness, egui::Role::ComboBox, text::MODEL);
    harness.snapshot("inspector_ai_codex_models");
    harness.get_by_label("Codex Fast").click();
    harness.run();
    assert!(
        harness
            .state()
            .editor
            .ai
            .edits
            .contains(&AiEdit::Model(Some("codex-fast".into())))
    );
    harness.state_mut().editor.ai.choices.model = Some("codex-fast".into());
    harness.run();
    harness.snapshot("inspector_ai_codex");
    click(&mut harness, egui::Role::ComboBox, text::MODEL);
    harness.get_by_label(text::MODEL_DEFAULT).click();
    harness.run();
    assert!(
        harness
            .state()
            .editor
            .ai
            .edits
            .contains(&AiEdit::Model(None))
    );
    harness.state_mut().editor.ai.signed_in = Some(false);
    harness.run();
    assert!(
        harness
            .query_by_label(text::CODEX_SIGNED_OUT_TEXT)
            .is_some()
    );
    harness.snapshot("inspector_ai_codex_signed_out");
}

#[test]
fn a_custom_codex_model_stays_available_and_catalog_errors_allow_refresh() {
    let mut harness = ai_tab(vec![Found {
        agent: Agent::Codex,
        path: "missing-codex.exe".into(),
    }]);
    harness.state_mut().groups.0.insert("ai.agent".into(), true);
    harness.state_mut().editor.ai.choices.model = Some("custom-codex".into());
    harness.state_mut().editor.ai.models.error = Some("catalog unavailable".into());
    harness.run();
    click(&mut harness, egui::Role::ComboBox, text::MODEL);
    assert!(harness.query_by_label("custom-codex").is_some());
    harness.get_by_label(text::MODEL_DEFAULT).click();
    harness.run();
    click(&mut harness, egui::Role::Button, text::CHECK_AGAIN);
    assert_ne!(
        harness.state().editor.ai.models.error.as_deref(),
        Some("catalog unavailable")
    );
}
