use dc_capture::input::{Button, CursorEvent, KeyEvent, Modifiers};

use crate::test_util::Recordings;

fn down(button: Button, x: i32, y: i32) -> CursorEvent {
    CursorEvent::Down { button, x, y }
}

fn up(x: i32, y: i32) -> CursorEvent {
    CursorEvent::Up {
        button: Button::Left,
        x,
        y,
    }
}

fn key(key: u16, ctrl: bool) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers {
            ctrl,
            ..Modifiers::default()
        },
    }
}

/// Two recordings, 20 s and 10 s, 1920×1080: clicks, a double click, keys.
fn project() -> Recordings {
    let f = Recordings::with_telemetry(&[20.0, 10.0]);
    f.cursor(
        1,
        &[
            (0.0, CursorEvent::Move { x: 900, y: 500 }),
            (0.5, CursorEvent::Move { x: 960, y: 540 }),
            (1.0, down(Button::Left, 960, 540)),
            (1.1, up(960, 540)),
            (1.3, down(Button::Left, 961, 541)),
            (5.0, down(Button::Right, 192, 108)),
            (
                5.2,
                CursorEvent::Wheel {
                    x: 192,
                    y: 108,
                    delta: -120,
                },
            ),
        ],
    );
    f.keys(1, &[(3.0, key(u16::from(b'S'), true))]);
    f.cursor(2, &[(2.0, down(Button::Left, 1920, -108))]);
    f.keys(2, &[(1.0, key(0x74, false))]);
    f
}

#[test]
fn clicks_are_in_output_time_and_frame_fractions() {
    let f = project();
    assert_eq!(
        f.ok("events clicks"),
        "0:01.000  left  x=0.5  y=0.5\n\
         0:01.300  left double  x=0.501  y=0.501\n\
         0:05.000  right  x=0.1  y=0.1\n\
         0:22.000  left  x=1  y=-0.1"
    );
    f.ok("cut 4-6");
    assert_eq!(
        f.ok("events clicks --from 1.2"),
        "0:01.300  left double  x=0.501  y=0.501\n\
         0:20.000  left  x=1  y=-0.1\n\
         вырезано и не показано событий: 1"
    );
    assert_eq!(
        f.ok("events clicks --to 1.3"),
        "0:01.000  left  x=0.5  y=0.5\nвырезано и не показано событий: 1"
    );
    let json: serde_json::Value = serde_json::from_str(&f.ok("events clicks --json")).unwrap();
    assert_eq!(json["total"], 3);
    assert_eq!(json["cut"], 1);
    assert_eq!(
        json["items"][2],
        serde_json::json!({"at": 20.0, "clip": "c2", "button": "left", "double": false, "x": 1.0, "y": -0.1})
    );
}

#[test]
fn positions_follow_the_crop_of_the_clip() {
    let f = project();
    f.ok("crop c1 0.5,0.5,0.5,0.5");
    assert_eq!(
        f.ok("events clicks --to 1.1"),
        "0:01.000  left  x=0  y=0",
        "the crop corner is 0,0"
    );
}

#[test]
fn keys_and_activity_on_two_clips() {
    let f = project();
    assert_eq!(f.ok("events keys"), "0:03.000  Ctrl+S\n0:21.000  F5");
    assert_eq!(
        f.ok("events activity"),
        "0:00.500–0:01.300  moves=2  clicks=2\n\
         0:03.000–0:03.000  keys=1\n\
         0:05.000–0:05.200  clicks=1  wheel=1\n\
         0:21.000–0:22.000  clicks=1  keys=1"
    );
    assert_eq!(
        f.ok("events activity --from 3.5 --to 21"),
        "0:05.000–0:05.200  clicks=1  wheel=1"
    );
    f.ok("clip move c2 --to 1");
    assert_eq!(f.ok("events keys"), "0:01.000  F5\n0:13.000  Ctrl+S");
}

#[test]
fn empty_lists_and_wrong_kind() {
    let f = Recordings::with_telemetry(&[5.0]);
    assert_eq!(f.ok("events keys"), "keys: пусто");
    assert_eq!(f.run("events taps").code, 2);
    assert_eq!(f.run("events clicks --from soon").code, 2);
}
