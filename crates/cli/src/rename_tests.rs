//! `devcast rename`: the folder gets the new name with everything in it, a taken name and a
//! project held by the editor are refused, `--dry-run` only says what would happen.

use dc_project::ProjectLock;

use crate::test_util::Fixture;

#[test]
fn renames_the_folder_of_the_project() {
    let f = Fixture::new();
    f.run("test add --secs 2");
    let out = f.run("rename Урок 2 с пробелами");
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        out.stdout.trim(),
        "«Урок 1.devcast» → «Урок 2 с пробелами.devcast»"
    );
    assert!(!f.dir.root().exists());
    let renamed = dc_project::ProjectDir::open(f.cwd.join("Урок 2 с пробелами.devcast")).unwrap();
    assert_eq!(renamed.load().unwrap().clips.len(), 1);
}

#[test]
fn dry_run_and_json_say_where_it_goes_and_change_nothing() {
    let f = Fixture::new();
    let out = f.run("rename Урок 2 --dry-run --json");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["to"], "Урок 2.devcast");
    assert_eq!(json["dry_run"], true);
    assert!(f.dir.root().is_dir());
}

#[test]
fn refuses_a_taken_name_and_a_project_the_editor_holds() {
    let f = Fixture::new();
    dc_project::ProjectDir::create(f.cwd.join("Занято.devcast")).unwrap();
    let out = f.run_in("rename Занято", f.dir.root());
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("уже существует"), "{}", out.stderr);
    let _editor = ProjectLock::acquire(&f.dir, "devcast-app").unwrap();
    let out = f.run_in("rename Свободно", f.dir.root());
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("проект уже открыт: devcast-app"),
        "{}",
        out.stderr
    );
    assert!(f.dir.root().is_dir(), "nothing moved");
}
