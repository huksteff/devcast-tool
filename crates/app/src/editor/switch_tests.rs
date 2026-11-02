use std::path::PathBuf;
use std::time::Instant;

use dc_project::ProjectDir;
use dc_project::dir::SESSION_FILE;
use eframe::egui;

use super::*;
use crate::settings::Store;

/// Two saved projects `А.devcast` and `Б.devcast` in a temporary folder, `А` with unsaved changes
/// left (a `session.json`); a `Shared` with settings of its own.
fn two() -> (tempfile::TempDir, Shared, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let project = crate::editor::tests::project();
    let roots: Vec<PathBuf> = ["А", "Б"]
        .iter()
        .map(|name| {
            let dir = ProjectDir::create(temp.path().join(format!("{name}.devcast"))).unwrap();
            dir.save(&project).unwrap();
            dir.root().to_path_buf()
        })
        .collect();
    ProjectDir::open(&roots[0])
        .unwrap()
        .save_session(&project)
        .unwrap();
    let store = Store::load(&temp.path().join("settings.json")).unwrap();
    let shared = Shared::new(egui::Context::default(), store);
    (temp, shared, roots[0].clone(), roots[1].clone())
}

fn open_root(shared: &Shared) -> PathBuf {
    shared
        .editor
        .as_ref()
        .unwrap()
        .doc
        .dir()
        .root()
        .to_path_buf()
}

/// Shows keys in the open project, not written to disk yet.
fn edit(shared: &mut Shared) {
    let step = dc_ops::Step::new(
        "keys",
        serde_json::Map::from_iter([("show".to_owned(), true.into())]),
    );
    let editor = shared.editor.as_mut().unwrap();
    editor.doc.apply(&[step], Instant::now()).unwrap();
}

#[test]
fn a_working_state_left_on_disk_is_asked_about() {
    let (_temp, mut shared, a, b) = two();
    assert!(shared.open_project(&a, true));
    assert!(shared.editor.as_ref().unwrap().recovery);
    assert!(shared.open_project(&b, true));
    assert!(
        !shared.editor.as_ref().unwrap().recovery,
        "Б has nothing unsaved"
    );
}

#[test]
fn another_project_replaces_the_editor_and_keeps_its_edits_on_disk() {
    let (_temp, mut shared, a, b) = two();
    shared.open_project(&b, true);
    edit(&mut shared);
    shared.leave_project(Leave::Open(a.clone()));
    assert_eq!(open_root(&shared), a);
    assert!(
        b.join(SESSION_FILE).is_file(),
        "the edits of Б went to its session"
    );
    let recent = &shared.store.get().recent_projects;
    assert_eq!(recent.len(), 2);
    assert!(recent[0].ends_with("А.devcast") && recent[1].ends_with("Б.devcast"));
}

#[test]
fn a_held_project_is_a_toast_and_the_editor_stays() {
    let (_temp, mut shared, a, b) = two();
    shared.open_project(&a, false);
    let held = dc_project::ProjectLock::acquire(&ProjectDir::open(&b).unwrap(), "test").unwrap();
    assert!(!shared.open_project(&b, false));
    assert_eq!(open_root(&shared), a);
    assert!(!shared.toasts.is_empty());
    drop(held);
}

#[test]
fn recent_projects_are_newest_first_and_at_most_sixteen() {
    let (temp, mut shared, _, _) = two();
    let path = |n: usize| temp.path().join(format!("{n}.devcast"));
    for n in 0..20 {
        shared.remember_project(&path(n), None);
    }
    shared.remember_project(&path(5), None);
    let recent = &shared.store.get().recent_projects;
    assert_eq!(recent.len(), RECENT_MAX);
    assert_eq!(recent[0], path(5));
    assert_eq!(recent[1], path(19));
    assert_eq!(recent.iter().filter(|p| **p == path(5)).count(), 1);
}

#[test]
fn renaming_moves_the_folder_and_keeps_the_working_state() {
    let (_temp, mut shared, a, _b) = two();
    shared.open_project(&a, false);
    edit(&mut shared);
    let project = shared.editor.as_ref().unwrap().doc.project().clone();
    shared.leave_project(Leave::Rename("Урок 2".to_owned()));
    let renamed = a.with_file_name("Урок 2.devcast");
    assert!(shared.toasts.is_empty());
    assert!(!a.exists());
    assert_eq!(open_root(&shared), renamed);
    let editor = shared.editor.as_ref().unwrap();
    assert_eq!(editor.name, "Урок 2");
    assert!(editor.doc.unsaved() && !editor.recovery);
    assert_eq!(editor.doc.project(), &project);
    assert!(
        editor.doc.next(super::super::Way::Undo).is_some(),
        "undo goes on"
    );
    let recent = &shared.store.get().recent_projects;
    assert_eq!(recent.as_slice(), [renamed]);
}

#[test]
fn a_taken_name_is_refused_and_the_project_stays() {
    let (_temp, mut shared, a, _b) = two();
    shared.open_project(&a, false);
    shared.leave_project(Leave::Rename("Б".to_owned()));
    assert!(!shared.toasts.is_empty());
    assert_eq!(open_root(&shared), a);
}

#[test]
fn a_new_project_is_made_in_the_recordings_folder() {
    let (temp, mut shared, a, _b) = two();
    let folder = temp.path().join("Записи");
    shared
        .store
        .update(|s| s.recordings_dir = folder.clone())
        .unwrap();
    shared.open_project(&a, false);
    shared.leave_project(Leave::New);
    shared.leave_project(Leave::New);
    assert_eq!(open_root(&shared), folder.join("Новый проект 2.devcast"));
    assert!(folder.join("Новый проект.devcast").is_dir());
    assert!(
        shared
            .editor
            .as_ref()
            .unwrap()
            .doc
            .project()
            .clips
            .is_empty()
    );
}

#[test]
fn not_a_project_folder_is_refused() {
    let (temp, mut shared, a, _b) = two();
    shared.open_project(&a, false);
    shared.leave_project(Leave::Open(temp.path().to_path_buf()));
    assert_eq!(open_root(&shared), a);
    assert!(!shared.toasts.is_empty());
}

/// The requests of `devcast discard`, `close`, `save --as` and `rename` (`dc_ops::live`).
mod live {
    use dc_ops::live::{Answer, CHANGED, Changes, Request};

    use super::*;

    #[test]
    fn discard_drops_the_working_state() {
        let (_temp, mut shared, a, _b) = two();
        shared.open_project(&a, true);
        let request = || Request::Discard { root: a.clone() };
        assert_eq!(shared.live(request()), Answer::Discarded { changed: true });
        let editor = shared.editor.as_ref().unwrap();
        assert!(!editor.doc.unsaved() && !editor.recovery);
        assert!(!a.join(SESSION_FILE).exists());
        assert_eq!(shared.live(request()), Answer::Discarded { changed: false });
    }

    #[test]
    fn close_asks_what_to_do_with_unsaved_changes() {
        let (_temp, mut shared, a, b) = two();
        let close = |root: &PathBuf, changes| Request::Close {
            root: root.clone(),
            changes,
        };
        assert_eq!(
            shared.live(close(&b, None)),
            Answer::Elsewhere { open: None }
        );
        shared.open_project(&b, false);
        edit(&mut shared);
        let project = shared.editor.as_ref().unwrap().doc.project().clone();
        assert_eq!(shared.live(close(&b, None)), Answer::Unsaved);
        assert_eq!(
            shared.live(close(&a, None)),
            Answer::Elsewhere {
                open: Some(b.clone())
            }
        );
        assert_eq!(shared.live(close(&b, Some(Changes::Save))), Answer::Closed);
        assert!(shared.editor.is_none());
        assert!(!b.join(SESSION_FILE).exists());
        let saved = ProjectDir::open(&b).unwrap().load().unwrap();
        assert_eq!(saved, project, "saved before closing");
    }

    #[test]
    fn the_copy_of_save_as_opens_as_after_the_dialog() {
        let (temp, mut shared, _a, b) = two();
        shared.open_project(&b, false);
        edit(&mut shared);
        let editor = shared.editor.as_ref().unwrap();
        let (dir, project) = (editor.doc.dir().clone(), editor.doc.project().clone());
        let revision = editor.doc.revision();
        let copy = dir
            .save_as(&project, temp.path().join("В.devcast"))
            .unwrap();
        let copied = |revision| Request::Copied {
            root: b.clone(),
            revision,
            copy: copy.root().to_path_buf(),
        };
        let refused = shared.live(copied(revision + 1));
        assert_eq!(refused, Answer::Refused(CHANGED.to_owned()));
        assert_eq!(shared.live(copied(revision)), Answer::Opened);
        assert_eq!(open_root(&shared), copy.root());
        assert!(
            !b.join(SESSION_FILE).exists(),
            "Б dropped its working state"
        );
    }

    #[test]
    fn rename_answers_the_new_folder() {
        let (_temp, mut shared, a, b) = two();
        shared.open_project(&a, false);
        let rename = |root: &PathBuf, name: &str| Request::Rename {
            root: root.clone(),
            name: name.to_owned(),
        };
        assert_eq!(shared.live(rename(&b, "Урок 3")), Answer::NotOpen);
        let renamed = a.with_file_name("Урок 3.devcast");
        let answer = shared.live(rename(&a, "Урок 3"));
        assert_eq!(
            answer,
            Answer::Renamed {
                root: renamed.clone()
            }
        );
        assert_eq!(open_root(&shared), renamed);
        let taken = shared.live(rename(&renamed, "Б"));
        assert!(matches!(taken, Answer::Refused(_)), "{taken:?}");
    }

    /// `devcast config` and `devcast hotkeys` while the app runs: the edit goes through the one
    /// store (written at once), a refused one changes nothing.
    #[test]
    fn settings_edits_go_through_the_store() {
        use dc_settings::{Binding, Command, Edit, EditorKey, Setting, Shortcut};
        let (temp, mut shared, _a, _b) = two();
        let set = Edit::Set(vec![Setting::Fps(dc_capture::Fps::Fps30)]);
        let Answer::Configured(applied) = shared.live(Request::Settings { edit: set }) else {
            panic!("not configured");
        };
        assert_eq!(applied.keys, [dc_settings::Key::Fps]);
        assert_eq!(shared.store.get().recording.fps, dc_capture::Fps::Fps30);
        let file = Store::load(&temp.path().join("settings.json")).unwrap();
        assert_eq!(file.get(), shared.store.get());
        let taken = Edit::Bind {
            binding: Binding::Global(Command::StartStop),
            keys: Shortcut::parse("Ctrl+S"),
            swap: false,
        };
        let before = shared.store.get().clone();
        let refused = shared.live(Request::Settings { edit: taken });
        assert!(matches!(refused, Answer::NotConfigured(_)), "{refused:?}");
        assert_eq!(shared.store.get(), &before);
        let swapped = Edit::Bind {
            binding: Binding::Global(Command::StartStop),
            keys: Shortcut::parse("Ctrl+S"),
            swap: true,
        };
        shared.live(Request::Settings { edit: swapped });
        let keys = &shared.store.get().editor_keys;
        assert_eq!(keys.keys(EditorKey::Save).as_deref(), Some("Ctrl+Shift+R"));
    }

    #[test]
    fn current_names_the_project_of_the_editor() {
        let (_temp, mut shared, a, _b) = two();
        assert_eq!(
            shared.live(Request::Current),
            Answer::Current { root: None }
        );
        shared.open_project(&a, false);
        assert_eq!(
            shared.live(Request::Current),
            Answer::Current { root: Some(a) }
        );
    }
}
