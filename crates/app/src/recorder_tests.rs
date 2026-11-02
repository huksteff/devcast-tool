use std::path::Path;

use dc_fx_zoom::ZoomEffect;
use dc_project::{ProjectDir, ProjectLock};

use super::auto_zooms;

/// A copy of the fixture recording `clicks.devcast` in `parent`, as a stop leaves it.
fn clicks(parent: &Path, name: &str) -> ProjectDir {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.devcast");
    let source = ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    source.save_as(&project, parent.join(name)).unwrap()
}

#[test]
fn the_app_recording_gets_the_auto_zooms_of_devcast_record() {
    let temp = tempfile::tempdir().unwrap();
    let by_app = clicks(temp.path(), "запись программы.devcast");
    let by_cli = clicks(temp.path(), "запись devcast.devcast");
    auto_zooms(&by_app).unwrap();
    let lock = ProjectLock::acquire(&by_cli, "devcast").unwrap();
    dc_ops::autozoom::after_recording(&by_cli, &lock).unwrap();
    drop(lock);
    let (app, cli) = (by_app.load().unwrap(), by_cli.load().unwrap());
    assert!(!app.state::<ZoomEffect>().unwrap().zooms.is_empty());
    assert_eq!(
        serde_json::to_value(&app).unwrap(),
        serde_json::to_value(&cli).unwrap()
    );
    let history = |dir: &ProjectDir| std::fs::read_to_string(dir.root().join("history.jsonl"));
    assert_eq!(history(&by_app).unwrap(), history(&by_cli).unwrap());
    assert!(
        ProjectLock::acquire(&by_app, "devcast").is_ok(),
        "the lock is let go"
    );
}

#[test]
fn the_zoom_setting_off_adds_no_zooms() {
    let temp = tempfile::tempdir().unwrap();
    let dir = clicks(temp.path(), "без автозумов.devcast");
    let mut project = dir.load().unwrap();
    let mut state = project.state::<ZoomEffect>().unwrap();
    state.settings.auto = false;
    project.set_state::<ZoomEffect>(&state).unwrap();
    dir.save(&project).unwrap();
    auto_zooms(&dir).unwrap();
    assert_eq!(dir.load().unwrap(), project);
}
