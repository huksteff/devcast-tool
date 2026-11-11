use eframe::egui;

use super::{ID, settings_frame};
use crate::app::Shared;
use crate::downloads::Downloads;
use crate::models::Models;
use crate::settings::Store;
use crate::settings_defaults::DefaultsFile;
use crate::settings_window::{Page, SettingsWindow};

fn shared(dir: &tempfile::TempDir) -> Shared {
    let store = Store::load(&dir.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    let defaults = DefaultsFile::load(dir.path().join("defaults.json")).unwrap();
    let mut window = SettingsWindow::new(
        vec!["Микрофон".to_owned()],
        Models::new(None, Downloads::default()),
        defaults,
    );
    window.page = Page::Captions;
    shared.settings = Some(window);
    shared
}

/// A frame of the settings window; `minimized` as Windows reports it.
fn frame(shared: &mut Shared, minimized: bool) -> Vec<egui::ViewportCommand> {
    let id = egui::ViewportId::from_hash_of(ID);
    let mut input = egui::RawInput {
        viewport_id: id,
        ..Default::default()
    };
    let info = egui::ViewportInfo {
        minimized: Some(minimized),
        ..Default::default()
    };
    input.viewports.insert(id, info);
    let ctx = shared.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| settings_frame(ui, shared));
    output.textures_delta.clear();
    output
        .viewport_output
        .get(&id)
        .map(|out| out.commands.clone())
        .unwrap_or_default()
}

/// «Настройки» with the window open keeps what it shows and brings it up instead of making a
/// new one; a named page is turned to.
#[test]
fn open_window_is_brought_up_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(&dir);
    shared.open_settings();
    let window = shared.settings.as_ref().unwrap();
    assert_eq!(window.page, Page::Captions);
    assert_eq!(window.microphones, ["Микрофон"]);
    assert!(window.focus);
    shared.open_settings_at(Some(Page::Hotkeys));
    assert_eq!(shared.settings.as_ref().unwrap().page, Page::Hotkeys);
}

/// Windows ignores the focus asked of a minimized window: it is asked once the window is back.
#[test]
fn focus_waits_for_the_restored_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(&dir);
    shared.open_settings();
    let focus =
        |commands: &[egui::ViewportCommand]| commands.contains(&egui::ViewportCommand::Focus);
    assert!(!focus(&frame(&mut shared, true)));
    assert!(shared.settings.as_ref().unwrap().focus);
    assert!(focus(&frame(&mut shared, false)));
    assert!(!shared.settings.as_ref().unwrap().focus);
    assert!(!focus(&frame(&mut shared, false)));
}
