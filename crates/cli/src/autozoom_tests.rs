use dc_capture::input::{Button, CursorEvent};
use dc_fx_zoom::ZoomEffect;
use dc_ops::autozoom::after_recording;
use dc_project::ProjectLock;

use crate::test_util::Recordings;

fn click(secs: f64, x: i32, y: i32) -> [(f64, CursorEvent); 2] {
    [
        (
            secs,
            CursorEvent::Down {
                button: Button::Left,
                x,
                y,
            },
        ),
        (
            secs + 0.05,
            CursorEvent::Up {
                button: Button::Left,
                x,
                y,
            },
        ),
    ]
}

/// Recordings of 20 s and 10 s: a cluster of three clicks and a lone one in the first, one
/// click in the second.
fn project() -> Recordings {
    let f = Recordings::with_telemetry(&[20.0, 10.0]);
    let first = [
        click(2.0, 192, 108),
        click(3.0, 960, 540),
        click(4.0, 960, 540),
        click(12.0, 1440, 810),
    ];
    f.cursor(1, &first.concat());
    f.cursor(2, &click(1.0, 480, 270));
    f
}

#[test]
fn suggest_shows_zooms_and_changes_nothing() {
    let f = project();
    assert_eq!(
        f.ok("zoom suggest"),
        "0:01.500–0:04.500  clicks=3  x=0.1  y=0.1\n\
         0:11.500–0:12.500  clicks=1  x=0.75  y=0.75\n\
         0:20.500–0:21.500  clicks=1  x=0.25  y=0.25\n\
         добавить все: devcast zoom add --at clicks"
    );
    assert_eq!(f.ok("ls zooms"), "zooms: пусто (всего 0)");
    let json: serde_json::Value = serde_json::from_str(&f.ok("zoom suggest --json")).unwrap();
    assert_eq!(json["clicks"], 5);
    assert_eq!(json["items"][2]["from"], 20.5);
}

#[test]
fn add_at_clicks_is_one_undo_step() {
    let f = project();
    assert_eq!(
        f.ok("zoom add --at clicks --dry-run"),
        "dry-run, проект не изменён: зумы +3 z1..z3"
    );
    assert_eq!(f.ok("zoom add --at clicks"), "зумы +3 z1..z3");
    assert_eq!(
        f.ok("ls zooms --fields scale,mode"),
        "z1  0:01.500–0:04.500  mode=follow  scale=1.5\n\
         z2  0:11.500–0:12.500  mode=follow  scale=1.5\n\
         z3  0:20.500–0:21.500  mode=follow  scale=1.5"
    );
    assert_eq!(
        f.ok("zoom suggest"),
        "автозумов нет: у всех групп кликов (3) уже есть зумы"
    );
    let out = f.run("zoom add --at clicks");
    assert_eq!(out.stdout, "без изменений\n");
    assert_eq!(
        out.stderr,
        "внимание: автозумов нет: у всех групп кликов (3) уже есть зумы\n"
    );
    assert_eq!(f.ok("undo"), "отменено: зумы +3 z1..z3");
    f.ok("cut 2.5-3.5");
    f.ok("zoom add --at 11 --duration 2");
    assert_eq!(
        f.ok("zoom add --at clicks --scale 2 --mode fixed"),
        "зумы +2 z5,z6",
        "the cut click is hidden, the taken cluster is skipped"
    );
    assert_eq!(
        f.ok("ls zooms --fields scale,mode,focus"),
        "z5  0:01.500–0:03.500  focus.x=0.1  focus.y=0.1  mode=fixed  scale=2.0\n\
         z4  0:11.000–0:13.000  focus.x=0.5  focus.y=0.5  mode=follow  scale=1.5\n\
         z6  0:19.500–0:20.500  focus.x=0.25  focus.y=0.25  mode=fixed  scale=2.0"
    );
}

#[test]
fn add_at_clicks_takes_only_scale_and_mode() {
    let f = project();
    let out = f.run("zoom add --at clicks --duration 2");
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("только --scale и --mode"),
        "{}",
        out.stderr
    );
    assert_eq!(f.run("zoom add --at clicks --scale 9").code, 1);
    assert_eq!(f.ok("zoom add --at 1"), "зумы +1 z1", "times still work");
}

#[test]
fn without_clicks_nothing_is_suggested() {
    let f = Recordings::with_telemetry(&[5.0]);
    assert_eq!(f.ok("zoom suggest"), "автозумов нет: в записях нет кликов");
}

#[test]
fn a_recording_gets_auto_zooms_unless_the_setting_is_off() {
    let f = project();
    let lock = ProjectLock::acquire(&f.dir, "devcast").unwrap();
    let added = after_recording(&f.dir, &lock).unwrap().unwrap();
    assert_eq!(added.to_string(), "зумы +3 z1..z3");
    drop(lock);
    f.ok("set zoom.auto=false");
    let lock = ProjectLock::acquire(&f.dir, "devcast").unwrap();
    assert!(after_recording(&f.dir, &lock).unwrap().is_none());
    drop(lock);
    let history = f.ok("history");
    assert!(history.contains("  zoom add  зумы +3 z1..z3"), "{history}");
    assert_eq!(
        f.dir
            .load()
            .unwrap()
            .state::<ZoomEffect>()
            .unwrap()
            .zooms
            .len(),
        3
    );
}

/// `zoom add --at clicks` on the fixture recording `clicks.devcast` adds exactly the zooms of
/// `dc_ops::click_zooms` — the path the editor's «Подобрать зумы» takes.
#[test]
fn clicks_of_the_fixture_are_the_zooms_of_dc_ops() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Подбор зумов.devcast");
    crate::test_util::fixture_copy("clicks.devcast", &root);
    let dir = dc_project::ProjectDir::open(&root).unwrap();
    let mut expected = dir.load().unwrap();
    let found = dc_ops::click_zooms(&root, &expected).unwrap();
    let commands = dc_ops::click_commands(&found, dc_fx_zoom::DEFAULT_SCALE, Default::default());
    let name = <dc_fx_zoom::ZoomAdd as dc_core::Op>::NAME;
    let mut history = dc_core::History::default();
    let commands = dc_ops::as_commands(&commands);
    dc_ops::apply_commands(&mut expected, &mut history, name, &commands, false).unwrap();
    let defaults = crate::test_util::defaults_file(tmp.path());
    let registry = dc_ops::Registry::devcast().unwrap();
    let suggest = crate::test_util::run_line_with("zoom suggest", &root, &registry, &defaults);
    insta::assert_snapshot!(suggest.stdout);
    let out = crate::test_util::run_line_with("zoom add --at clicks", &root, &registry, &defaults);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let added = dc_project::ProjectDir::open(&root).unwrap().load().unwrap();
    assert_eq!(
        added.state::<ZoomEffect>().unwrap().zooms.len(),
        found.zooms.len()
    );
    assert_eq!(
        serde_json::to_value(&added).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn clicks_in_a_plan_add_the_same_zooms_as_the_op() {
    let (by_op, by_plan) = (project(), project());
    assert_eq!(by_op.ok("zoom add --at clicks --scale 2"), "зумы +3 z1..z3");
    let plan = by_plan.dir.root().join("клики.json");
    let steps = r#"[{"op": "zoom add", "at": "clicks", "scale": 2}, {"op": "zoom add", "at": 25}]"#;
    std::fs::write(&plan, steps).unwrap();
    assert_eq!(by_plan.ok("apply клики.json"), "зумы +4 z1..z4");
    by_op.ok("zoom add --at 25");
    let zooms = |f: &Recordings| f.dir.load().unwrap().state::<ZoomEffect>().unwrap();
    assert_eq!(zooms(&by_plan), zooms(&by_op));
}
