//! `set`, `defaults`, `style copy` and `batch` on projects with the real registry; the defaults
//! file lies in the test's temporary folder.

use dc_core::Project;
use dc_fx_captions::CaptionsEffect;
use dc_fx_cursor::Cursor;
use dc_fx_zoom::{Preset, ZoomEffect};
use dc_project::{Defaults, ProjectDir};

use crate::test_util::{Recordings, defaults_file};

fn sibling(f: &Recordings, name: &str) -> ProjectDir {
    let dir = ProjectDir::create(f.dir.root().parent().unwrap().join(name)).unwrap();
    dir.save(&Project::default()).unwrap();
    dir
}

#[test]
fn set_changes_settings_through_their_commands_in_one_step() {
    let f = Recordings::new();
    let before = f.dir.load().unwrap();
    assert_eq!(
        f.ok("set captions.size=36 zoom.preset=smooth cursor.size=3"),
        "субтитры: размер 30→36; движение зума: вход 0.200→1.523 с, выход 0.200→1.015 с (Smooth); курсор: размер 2.50→3.00"
    );
    let project = f.dir.load().unwrap();
    assert_eq!(
        project.state::<CaptionsEffect>().unwrap().settings.size,
        36.0
    );
    let zoom = project.state::<ZoomEffect>().unwrap().settings;
    assert_eq!(zoom.preset(), Some(Preset::Smooth));
    assert!(f.ok("history").contains("1  set "), "one step");
    f.ok("undo");
    assert_eq!(f.dir.load().unwrap(), before);

    let other = Recordings::new();
    other.ok("cursor --size 3");
    f.ok("set cursor.size=3");
    assert_eq!(
        f.dir.load().unwrap().effects,
        other.dir.load().unwrap().effects,
        "set = the effect's own command"
    );
}

#[test]
fn set_keeps_the_checks_of_the_commands() {
    let f = Recordings::new();
    let before = f.dir.load().unwrap();
    for (line, code, hint) in [
        ("set captions.size=100", 1, "от 16 до 72"),
        ("set captions.size=36 cursor.size=99", 1, "размер курсора"),
        ("set zoom.foo=1", 2, "unknown field `foo`"),
        ("set focus.x=1", 2, "нет настроек «focus»: есть crop, zoom"),
        ("set captions.size", 2, "настройка.параметр=значение"),
    ] {
        let out = f.run(line);
        assert_eq!(out.code, code, "{line}: {}", out.stderr);
        assert!(out.stderr.contains(hint), "{line}: {}", out.stderr);
    }
    assert!(
        f.ok("set crop.aspect=16:9 --dry-run")
            .starts_with("dry-run")
    );
    assert_eq!(f.dir.load().unwrap(), before, "nothing changed");
    assert!(f.ok("help set").contains("captions — captions style"));
}

#[test]
fn defaults_are_set_shown_taken_from_a_project_and_reset() {
    let f = Recordings::new();
    let file = defaults_file(f.dir.root().parent().unwrap());
    assert_eq!(
        f.ok("defaults show"),
        "все настройки по умолчанию заводские"
    );
    f.ok("defaults set zoom.preset=smooth captions.size=36 --dry-run");
    assert!(!file.exists());
    assert_eq!(
        f.ok("defaults set captions.size=36"),
        "по умолчанию: субтитры: размер 30→36"
    );
    assert_eq!(
        f.ok("defaults show"),
        "captions.size=36.0\nостальные — заводские"
    );
    assert_eq!(f.run("defaults set captions.size=10").code, 1);
    let mut fresh = Project::default();
    Defaults::load(&file).unwrap().apply(&mut fresh);
    assert_eq!(fresh.state::<CaptionsEffect>().unwrap().settings.size, 36.0);

    f.ok("cursor --size 3");
    assert_eq!(
        f.ok("defaults from-project"),
        "по умолчанию — настройки «Урок 2.dcproj»: cursor, остальные заводские"
    );
    assert_eq!(
        f.ok("defaults show"),
        "cursor.size=3.0\nостальные — заводские"
    );
    f.ok("defaults reset");
    assert!(!file.exists());
    assert_eq!(f.run("batch . -- defaults show").code, 2);
}

#[test]
fn style_copy_replaces_settings_and_keeps_content() {
    let f = Recordings::new();
    f.ok("set captions.size=40");
    let target = sibling(&f, "Другой.dcproj");
    let mut project = Project::default();
    let mut cursor = project.state::<Cursor>().unwrap();
    cursor.settings.size = 3.0;
    project.set_state::<Cursor>(&cursor).unwrap();
    target.save(&project).unwrap();

    let line = "style copy --from . --to ../Другой*.dcproj";
    assert!(f.ok(&format!("{line} --dry-run")).contains("dry-run"));
    assert_eq!(target.load().unwrap(), project);
    assert_eq!(
        f.ok(line),
        "Другой.dcproj: стиль из «Урок 2.dcproj»: captions, cursor\nготово в 1 из 1"
    );
    let copied = target.load().unwrap();
    assert_eq!(
        copied.state::<CaptionsEffect>().unwrap().settings.size,
        40.0
    );
    assert_eq!(copied.state::<Cursor>().unwrap().settings.size, 2.5);
    assert_eq!(
        f.ok("style copy --from . --to ../*.dcproj"),
        "Другой.dcproj: без изменений\nУрок 2.dcproj: без изменений\nготово в 2 из 2"
    );
    let history = f.ok("history --project ../Другой.dcproj");
    assert!(history.contains("style copy"), "{history}");
}

#[test]
fn batch_runs_one_command_in_every_project() {
    let f = Recordings::new();
    let other = sibling(&f, "Второй.dcproj");
    assert_eq!(
        f.ok("batch ../*.dcproj -- set cursor.size=3"),
        "Второй.dcproj: курсор: размер 2.50→3.00\nУрок 2.dcproj: курсор: размер 2.50→3.00\nготово в 2 из 2"
    );
    for dir in [&f.dir, &other] {
        let project = dir.load().unwrap();
        assert_eq!(project.state::<Cursor>().unwrap().settings.size, 3.0);
    }
    let out = f.run("batch ../*.dcproj -- clip rm c2");
    assert_eq!(out.code, 1, "the empty project has no c2");
    assert!(
        out.stdout.contains("Урок 2.dcproj: клипы -1 c2"),
        "{}",
        out.stdout
    );
    assert!(
        out.stderr.contains("Второй.dcproj: не получилось"),
        "{}",
        out.stderr
    );
    for (line, code) in [
        ("batch ../*.dcproj -- batch . -- info", 2),
        ("batch ../*.dcproj -- export -o a.mp4", 2),
        ("batch ../*.dcproj -- nope", 2),
        ("batch ../*.dcproj -- info --project .", 2),
        ("batch ../*/x.dcproj -- info", 2),
        ("batch ../нет*.dcproj -- info", 1),
    ] {
        assert_eq!(f.run(line).code, code, "{line}");
    }
}
