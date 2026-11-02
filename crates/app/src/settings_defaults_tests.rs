use dc_core::History;
use dc_ops::Registry;
use dc_ui_kit::strings::settings as text;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::downloads::Downloads;
use crate::hotkeys::Command;
use crate::models::Models;
use crate::settings::Store;
use crate::settings_window::{Page, SIZE, SettingsWindow};

struct State {
    window: SettingsWindow,
    store: Store,
    /// The project of the editor, `None` — no editor open.
    project: Option<Project>,
    /// The defaults of the editor.
    defaults: Defaults,
    dir: tempfile::TempDir,
}

/// The project after `devcast set <assignments>`: the CLI's settings commands.
fn styled(assignments: &[&str]) -> Project {
    let registry = Registry::devcast().unwrap();
    let assignments: Vec<String> = assignments.iter().map(|a| (*a).to_owned()).collect();
    let commands = dc_ops::settings_commands(&registry, &assignments).unwrap();
    let commands: Vec<&dyn dc_core::Command> = commands.iter().map(AsRef::as_ref).collect();
    let mut project = Project::default();
    dc_ops::apply_commands(
        &mut project,
        &mut History::default(),
        "set",
        &commands,
        false,
    )
    .unwrap();
    project
}

/// Where the tests keep the defaults: a temporary folder, never `%APPDATA%`.
fn file(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("Настройки DevCast").join("defaults.json")
}

fn harness(project: Option<Project>) -> egui_kittest::Harness<'static, State> {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::load(&dir.path().join("settings.json")).unwrap();
    let defaults = DefaultsFile::load(file(&dir)).unwrap();
    let mut window = SettingsWindow::new(
        Vec::new(),
        Models::new(None, Downloads::default()),
        defaults,
    );
    window.page = Page::Defaults;
    let state = State {
        window,
        store,
        project,
        defaults: Defaults::factory(),
        dir,
    };
    dc_ui_kit::testing::harness()
        .with_size(SIZE)
        .build_ui_state(
            |ui, state: &mut State| {
                let taken: &[Command] = &[];
                let open = state.project.as_ref().map(|project| OpenProject {
                    name: "Демо-запись",
                    project,
                    defaults: &mut state.defaults,
                });
                let store = &mut state.store;
                state.window.ui(ui, store, (0.0, taken), open).unwrap();
            },
            state,
        )
}

#[test]
fn save_and_reset_write_the_file_as_the_cli() {
    let project = styled(&["zoom.preset=smooth", "captions.size=36"]);
    let mut harness = harness(Some(project.clone()));
    harness.run();
    harness.snapshot("settings_defaults");
    harness.get_by_label(text::SAVE).click();
    harness.run();
    let path = file(&harness.state().dir);
    let by_cli = harness.state().dir.path().join("cli").join("defaults.json");
    // `devcast defaults from-project` writes `Defaults::from_project` of the working project.
    Defaults::from_project(&project).save(&by_cli).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        std::fs::read(&by_cli).unwrap()
    );
    let saved = Defaults::load(&path).unwrap();
    assert_ne!(saved, Defaults::factory());
    assert_eq!(harness.state().defaults, saved, "the editor resets to them");
    assert_eq!(harness.state().window.defaults.current, saved);
    harness.get_by_label(text::OWN_NOW);
    harness.get_by_label(text::RESET).click();
    harness.run();
    // `devcast defaults reset` deletes the file.
    assert!(!path.exists());
    assert_eq!(harness.state().defaults, Defaults::factory());
    assert_eq!(harness.state().window.defaults.current, Defaults::factory());
    harness.get_by_label(text::FACTORY_NOW);
}

#[test]
fn without_an_editor_only_reset_works() {
    let mut harness = harness(None);
    let path = file(&harness.state().dir);
    Defaults::from_project(&styled(&["captions.size=36"]))
        .save(&path)
        .unwrap();
    let loaded = DefaultsFile::load(path.clone()).unwrap();
    harness.state_mut().window.defaults = loaded;
    harness.run();
    harness.get_by_label(text::NO_PROJECT);
    harness.get_by_label(text::SAVE).click();
    harness.run();
    assert!(path.exists(), "«Сохранить» is off without a project");
    harness.get_by_label(text::RESET).click();
    harness.run();
    assert!(!path.exists());
}
