use super::TrayView;

fn view(recording: bool) -> TrayView {
    TrayView {
        status: String::new(),
        recording,
        can_stop: recording,
    }
}

#[test]
fn settings_are_inactive_while_recording() {
    assert!(view(false).can_open_settings());
    assert!(!view(true).can_open_settings());
}
