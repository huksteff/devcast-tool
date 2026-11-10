use super::TrayView;

fn view(recording: bool) -> TrayView {
    TrayView {
        status: String::new(),
        recording,
        can_stop: recording,
        recent: Vec::new(),
    }
}

#[test]
fn settings_are_inactive_while_recording() {
    assert!(view(false).can_open_settings());
    assert!(!view(true).can_open_settings());
}

#[test]
fn a_recent_project_item_names_its_place() {
    use super::{TrayAction, opened};
    assert_eq!(opened("recent-3"), Some(TrayAction::Open(3)));
    assert_eq!(opened("recent-"), None);
    assert_eq!(opened("42"), None);
}
