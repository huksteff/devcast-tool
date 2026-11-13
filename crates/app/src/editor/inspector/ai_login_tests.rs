//! Codex uses a browser callback, while Claude can ask for a pasted code.

#[cfg(windows)]
use super::*;

#[cfg(windows)]
#[test]
fn codex_login_waits_for_the_browser_without_asking_for_a_code() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("login.cmd");
    std::fs::write(&script, "@set /p waiting=\r\n").unwrap();
    for agent in [Agent::Claude, Agent::Codex] {
        let mut harness = ai_tab(vec![Found {
            agent,
            path: script.clone(),
        }]);
        let ctx = harness.ctx.clone();
        let ai = &mut harness.state_mut().editor.ai;
        ai.signed_in = Some(false);
        ai.login(&ctx).unwrap();
        // The login is active: inspect its frame without asserting that repainting stopped.
        harness.step();
        assert!(harness.query_by_label(text::CANCEL_SIGN_IN).is_some());
        let has_code = harness.query_by_label(text::SEND_CODE).is_some();
        assert_eq!(has_code, agent == Agent::Claude);
        if agent == Agent::Codex {
            assert!(
                harness
                    .query_by_label(text::CODEX_SIGNING_IN_TEXT)
                    .is_some()
            );
            harness.snapshot("inspector_ai_codex_login");
        }
        harness.state_mut().editor.ai.cancel_login();
    }
}
