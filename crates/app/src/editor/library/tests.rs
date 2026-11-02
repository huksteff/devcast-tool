//! The library through the window: search, picking, + / double click / «На таймлайн» / a drag
//! = the project of `devcast clip add <project>` (`dc_ops::import_clips`), one undo step; the
//! Recycle Bin and «Отменить»; «Импорт…» = `devcast new`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use dc_core::History;
use dc_project::ProjectDir;
use dc_ui_kit::strings::{editor as editor_text, library as text};
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;

use super::Ask;
use crate::editor::editing_tests::{json, undone};
use crate::editor::inspector::tests::{H, project_json, undo};
use crate::editor::tests::harness;
use crate::editor::{Action, SIZE, Way};

/// Recordings `урок 1` (newest), `урок 2` and `демо` (oldest), copies of the fixtures.
struct Folder {
    _tmp: tempfile::TempDir,
    path: PathBuf,
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings")
        .join(name)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        match entry.file_type().unwrap().is_dir() {
            true => copy_dir(&entry.path(), &target),
            false => drop(std::fs::copy(entry.path(), &target).unwrap()),
        }
    }
}

impl Folder {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Записи DevCast");
        let epoch = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        for (age, (source, name)) in [
            ("clicks.devcast", "урок 1"),
            ("typing.devcast", "урок 2"),
            ("window.devcast", "демо"),
        ]
        .into_iter()
        .enumerate()
        {
            let root = path.join(format!("{name}.devcast"));
            copy_dir(&fixture(source), &root);
            let file = std::fs::File::options()
                .write(true)
                .open(root.join("project.json"))
                .unwrap();
            file.set_modified(epoch - Duration::from_secs(age as u64 * 60))
                .unwrap();
        }
        Self { _tmp: tmp, path }
    }

    fn root(&self, name: &str) -> PathBuf {
        self.path.join(format!("{name}.devcast"))
    }
}

/// The editor with the library of `folder` open.
fn opened(folder: &Folder) -> H {
    let mut harness = harness(SIZE);
    let editor = &mut harness.state_mut().editor;
    let open = editor.doc.dir().root().to_path_buf();
    editor.library.folder(&folder.path, &open);
    harness.run();
    harness.get_by_label(editor_text::LIBRARY).click();
    harness.run();
    harness
}

fn tile(harness: &H, name: &str) -> Pos2 {
    harness.get_by_label(name).rect().center()
}

/// `times` clicks on the tile `name` with `modifiers` held.
fn click(harness: &mut H, name: &str, modifiers: Modifiers, times: usize) {
    let at = tile(harness, name);
    harness.event(Event::ModifiersChanged(modifiers));
    harness.event(Event::PointerMoved(at));
    for pressed in [true, false].repeat(times) {
        harness.event(Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    harness.run();
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
}

fn picked(harness: &H) -> Vec<PathBuf> {
    harness.state().editor.library.picked.clone()
}

/// The project before `added` plus `clip add` of them, as `devcast clip add` makes it.
fn by_cli(before: &dc_core::Project, dir: &ProjectDir, added: &[PathBuf]) -> serde_json::Value {
    let import = dc_ops::import_clips(dir, added, true).unwrap();
    let mut project = before.clone();
    dc_ops::apply_commands(
        &mut project,
        &mut History::default(),
        "clip add",
        &dc_ops::as_commands(&import.commands),
        false,
    )
    .unwrap();
    json(&project)
}

#[test]
fn the_panel_searches_and_picks() {
    let folder = Folder::new();
    let mut harness = opened(&folder);
    harness.snapshot("library");
    harness.state_mut().editor.library.search = "УРОК".to_owned();
    harness.run();
    assert!(harness.query_by_label("демо").is_none());
    assert!(harness.query_by_label("урок 2").is_some());
    harness.state_mut().editor.library.search.clear();
    harness.run();

    click(&mut harness, "урок 1", Modifiers::NONE, 1);
    assert_eq!(picked(&harness), [folder.root("урок 1")]);
    click(&mut harness, "демо", Modifiers::COMMAND, 1);
    assert_eq!(
        picked(&harness),
        [folder.root("урок 1"), folder.root("демо")]
    );
    assert!(harness.query_by_label(&text::picked(2)).is_some());
    click(&mut harness, "урок 1", Modifiers::COMMAND, 1);
    assert_eq!(picked(&harness), [folder.root("демо")]);
    click(&mut harness, "демо", Modifiers::SHIFT, 1);
    let all = ["урок 1", "урок 2", "демо"].map(|n| folder.root(n));
    assert_eq!(picked(&harness), all);

    harness.key_press(eframe::egui::Key::Escape);
    harness.run();
    assert!(
        harness.query_by_label(text::TO_TIMELINE).is_none(),
        "Esc closes"
    );
}

#[test]
fn plus_double_click_button_and_drag_are_clip_add() {
    let folder = Folder::new();
    let mut harness = opened(&folder);
    let dir = harness.state().editor.doc.dir().clone();
    let before = harness.state().editor.doc.project().clone();
    let one = [folder.root("урок 2")];
    let expected = by_cli(&before, &dir, &one);
    let now = |harness: &H| harness.state().editor.doc.project().clone();

    // The harness clock runs a quarter second per frame and the second click comes a frame later.
    harness
        .ctx
        .options_mut(|o| o.input_options.max_double_click_delay = 1.0);
    click(&mut harness, "урок 2", Modifiers::NONE, 2);
    assert_eq!(project_json(&harness), expected);
    assert!(dir.media_dir().join("recording-1/cursor.bin").is_file());
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        undone(&before, &now(&harness)),
        "one step"
    );

    harness.get_by_label("урок 2").hover();
    harness.run();
    harness.get_by_label(&text::add_one("урок 2")).click();
    harness.run();
    let after_plus = harness.state().editor.doc.project().clone();
    assert_eq!(after_plus.clips.len(), before.clips.len() + 1);
    undo(&mut harness);

    click(&mut harness, "урок 1", Modifiers::NONE, 1);
    click(&mut harness, "демо", Modifiers::COMMAND, 1);
    let both = [folder.root("урок 1"), folder.root("демо")];
    let expected = by_cli(&now(&harness), &dir, &both);
    harness.get_by_label(text::TO_TIMELINE).click();
    harness.run();
    assert_eq!(project_json(&harness), expected);
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        undone(&before, &now(&harness)),
        "one step"
    );

    let expected = by_cli(&now(&harness), &dir, &one);

    let from = tile(&harness, "урок 2");
    let to = pos2(700.0, 700.0);
    harness.event(Event::PointerMoved(from));
    harness.event(Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        harness.event(Event::PointerMoved(from + (to - from) * share));
        harness.run();
    }
    harness.event(Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    harness.run();
    assert_eq!(project_json(&harness), expected);
}

#[test]
#[ignore = "moves folders through the real Windows Recycle Bin; run at night"]
fn the_bin_and_its_undo() {
    let folder = Folder::new();
    let mut harness = opened(&folder);
    let demo = folder.root("демо");
    click(&mut harness, "демо", Modifiers::NONE, 1);
    harness.get_by_label(text::TRASH).click();
    harness.run();
    assert!(!demo.exists());
    assert!(harness.query_by_label("демо").is_none());
    harness.snapshot("library_trashed");
    harness.get_by_label(text::UNDO).click();
    harness.run();
    assert!(demo.join("project.json").is_file());
    assert_eq!(picked(&harness), vec![demo.clone()]);

    harness.get_by_label(text::TRASH).click();
    harness.run();
    assert!(!demo.exists());
    let editor = &mut harness.state_mut().editor;
    editor.ask(Action::Travel(Way::Undo));
    harness.run();
    assert!(demo.join("project.json").is_file(), "Ctrl+Z brings it back");
    assert!(harness.state().editor.library.trashed.is_empty());
}

#[test]
fn import_makes_a_project_like_devcast_new() {
    let folder = Folder::new();
    let mut harness = opened(&folder);
    let video = folder.path.join("логотип.mp4");
    std::fs::copy(
        fixture("clicks.devcast/media/recording-1/video.mp4"),
        &video,
    )
    .unwrap();
    let editor = &mut harness.state_mut().editor;
    editor.ask(Action::Library(Ask::Import(vec![video.clone()])));
    harness.run();
    let root = folder.root("логотип");
    assert_eq!(picked(&harness), vec![root.clone()]);
    let made = ProjectDir::open(&root).unwrap().load().unwrap();
    let expected_root = folder.path.join("ожидание.devcast");
    let defaults = harness.state().editor.defaults.clone();
    dc_ops::new_project(&expected_root, &[video], &defaults, "test").unwrap();
    let expected = ProjectDir::open(&expected_root).unwrap().load().unwrap();
    assert_eq!(json(&made), json(&expected));
    assert!(harness.query_by_label("логотип").is_some());
}
