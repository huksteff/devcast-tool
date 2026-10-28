use dc_capture::input::{Button, CursorEvent};
use dc_fx_zoom::ZoomEffect;

use crate::autozoom::after_recording;
use crate::edit::Ctx;
use crate::globals::Globals;
use crate::registry::registry;
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
        "зумы +2 z2,z3",
        "the cut click is hidden, the taken cluster is skipped"
    );
    assert_eq!(
        f.ok("ls zooms --fields scale,mode,focus"),
        "z2  0:01.500–0:03.500  focus.x=0.1  focus.y=0.1  mode=fixed  scale=2.0\n\
         z1  0:11.000–0:13.000  focus.x=0.5  focus.y=0.5  mode=follow  scale=1.5\n\
         z3  0:19.500–0:20.500  focus.x=0.25  focus.y=0.25  mode=fixed  scale=2.0"
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
    let registry = registry().unwrap();
    let globals = Globals::default();
    let f = project();
    let ctx = Ctx {
        globals: &globals,
        cwd: f.dir.root(),
        registry: &registry,
    };
    let mut project = f.dir.load().unwrap();
    let added = after_recording(&ctx, &f.dir, &project).unwrap().unwrap();
    assert_eq!(added.text, "зумы +3 z1..z3");
    assert_eq!(
        f.ok("history"),
        "отменить (devcast undo N):\n  1  zoom add  зумы +3 z1..z3"
    );
    let mut state = project.state::<ZoomEffect>().unwrap();
    state.settings.auto = false;
    project.set_state::<ZoomEffect>(&state).unwrap();
    assert!(after_recording(&ctx, &f.dir, &project).unwrap().is_none());
}
