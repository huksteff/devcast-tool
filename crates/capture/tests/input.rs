//! Input hooks on the real desktop: synthetic input (SendInput) goes into the test's own
//! window on monitor `DEVCAST_TEST_MONITOR`, the recording is read back from its files.
//!
//! `#[ignore]`: moves the pointer (only over that monitor; put back where it was at the end).
//! `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --run-ignored=all --no-capture input`
//! The folder is deleted at the end; the numbers are printed. Not while the owner works: it
//! takes the pointer for a few seconds.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unsafe_code,
    reason = "test helpers: a failed step must fail the test; the test window is Win32"
)]

mod common;
#[path = "common/target.rs"]
mod target;

use std::path::PathBuf;
use std::time::Duration;

use common::synthetic::{self, PointerBack, chord_inputs, mouse, move_input};
use common::{fresh_dir, test_monitor};
use dc_capture::input::{
    Button, CursorEvent, CursorKind, KeyEvent, Modifiers, Timed, image_file, read_cursor, read_keys,
};
use dc_capture::win::{InputRecording, now};
use dc_capture::{HostTime, PauseClock};
use target::Target;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL,
};

const CLICKS: usize = 50;
const MOVES: i32 = 50;
/// How far from where it was sent a click may land: the window reaches 100 px around.
const NUDGE: i32 = 100;

/// The test window in the middle of the test monitor: its top-left corner.
fn show_target() -> (Target, i32, i32) {
    let bounds = test_monitor().bounds;
    let x = bounds.x() + bounds.width() as i32 / 2 - 200;
    let y = bounds.y() + bounds.height() as i32 / 2 - 150;
    (Target::show(x, y, 400, 300), x, y)
}

fn folder(name: &str) -> PathBuf {
    let dir = fresh_dir(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A button or wheel action at desktop pixel (`x`, `y`): the move and the action are one
/// `SendInput`, which the user's own mouse cannot get between.
fn at_point(x: i32, y: i32, flags: MOUSE_EVENT_FLAGS, data: i32) -> [INPUT; 2] {
    [move_input(x, y), mouse(flags, 0, 0, data)]
}

/// Sends `inputs` and returns the host time right before.
fn send(inputs: &[INPUT]) -> HostTime {
    let at = now().unwrap();
    synthetic::send(inputs);
    at
}

fn pause(millis: u64) {
    std::thread::sleep(Duration::from_millis(millis));
}

fn print_delays(downs: &[&Timed<CursorEvent>], pressed: &[HostTime]) {
    let mut delays: Vec<i64> = downs
        .iter()
        .zip(pressed)
        .map(|(e, sent)| e.at.micros() - sent.micros())
        .collect();
    delays.sort_unstable();
    println!(
        "sent: {MOVES} moves, {CLICKS} left clicks, 1 right, 1 middle, 3 wheel, 2 hwheel;          left down delay SendInput → hook: min {} µs, median {} µs, max {} µs",
        delays[0],
        delays[delays.len() / 2],
        delays[delays.len() - 1]
    );
}

fn count(events: &[Timed<CursorEvent>], f: impl Fn(&CursorEvent) -> bool) -> usize {
    events.iter().filter(|e| f(&e.event)).count()
}

/// Moves along a line in the window at (`x`, `y`), then clicks, scrolls there. Returns the
/// path and when each left button was pressed.
fn act(x: i32, y: i32) -> (Vec<(i32, i32)>, Vec<HostTime>) {
    let path: Vec<(i32, i32)> = (0..MOVES)
        .map(|i| (x + 20 + i * 5, y + 40 + i * 3))
        .collect();
    for &(px, py) in &path {
        send(&[move_input(px, py)]);
        pause(2);
    }
    let (end_x, end_y) = *path.last().unwrap();
    let mut pressed = Vec::new();
    for _ in 0..CLICKS {
        pressed.push(send(&at_point(end_x, end_y, MOUSEEVENTF_LEFTDOWN, 0)));
        pause(10);
        send(&at_point(end_x, end_y, MOUSEEVENTF_LEFTUP, 0));
        pause(20);
    }
    let others = [
        (MOUSEEVENTF_RIGHTDOWN, 0),
        (MOUSEEVENTF_RIGHTUP, 0),
        (MOUSEEVENTF_MIDDLEDOWN, 0),
        (MOUSEEVENTF_MIDDLEUP, 0),
        (MOUSEEVENTF_WHEEL, -120),
        (MOUSEEVENTF_WHEEL, -120),
        (MOUSEEVENTF_WHEEL, -120),
        (MOUSEEVENTF_HWHEEL, 120),
        (MOUSEEVENTF_HWHEEL, 120),
    ];
    for (flags, data) in others {
        send(&at_point(end_x, end_y, flags, data));
        pause(20);
    }
    (path, pressed)
}

#[test]
#[ignore = "moves the pointer over monitor DEVCAST_TEST_MONITOR"]
fn records_mouse_without_losses() {
    let _back = PointerBack::save();
    let (_target, x, y) = show_target();
    let dir = folder("input-mouse");
    let input = InputRecording::start().unwrap();
    input
        .begin(now().unwrap(), &dir, (0, 0), &PauseClock::default())
        .unwrap();
    let (path, pressed) = act(x, y);
    let (end_x, end_y) = *path.last().unwrap();
    // The user's last action is well before the stop.
    pause(200);
    let summary = input.join().unwrap();
    let events = read_cursor(&dir).unwrap();
    println!("summary {summary:?}, {} events in cursor.bin", events.len());
    assert!(events.windows(2).all(|w| w[0].at <= w[1].at));
    for &(px, py) in &path {
        let near = |e: &CursorEvent| {
            let Some((ex, ey)) = e.position() else {
                return false;
            };
            matches!(e, CursorEvent::Move { .. }) && (ex - px).abs() <= 1 && (ey - py).abs() <= 1
        };
        assert!(count(&events, near) >= 1, "move to ({px}, {py})");
    }
    // The owner's own mouse may nudge the pointer between the move and the click.
    let at_end = |e: &CursorEvent| {
        let Some((ex, ey)) = e.position() else {
            return false;
        };
        (ex - end_x).abs() <= NUDGE && (ey - end_y).abs() <= NUDGE
    };
    let button = |b: Button, down: bool| {
        move |e: &CursorEvent| match e {
            CursorEvent::Down { button, .. } => down && *button == b && at_end(e),
            CursorEvent::Up { button, .. } => !down && *button == b && at_end(e),
            _ => false,
        }
    };
    assert_eq!(count(&events, button(Button::Left, true)), CLICKS);
    assert_eq!(count(&events, button(Button::Left, false)), CLICKS);
    for b in [Button::Right, Button::Middle] {
        assert_eq!(count(&events, button(b, true)), 1);
        assert_eq!(count(&events, button(b, false)), 1);
    }
    let wheel = |e: &CursorEvent| matches!(e, CursorEvent::Wheel { delta: -120, .. });
    let hwheel = |e: &CursorEvent| matches!(e, CursorEvent::HWheel { delta: 120, .. });
    assert_eq!((count(&events, wheel), count(&events, hwheel)), (3, 2));
    let downs: Vec<_> = events
        .iter()
        .filter(|e| button(Button::Left, true)(&e.event))
        .collect();
    print_delays(&downs, &pressed);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "moves the pointer over monitor DEVCAST_TEST_MONITOR"]
fn records_cursor_shapes_and_pictures() {
    let _back = PointerBack::save();
    let (_target, x, y) = show_target();
    let dir = folder("input-shapes");
    let input = InputRecording::start().unwrap();
    input
        .begin(now().unwrap(), &dir, (0, 0), &PauseClock::default())
        .unwrap();
    for _ in 0..3 {
        for px in [x + 100, x + 300] {
            send(&[move_input(px, y + 150)]);
            pause(100);
        }
    }
    // The user's last action is well before the stop.
    pause(200);
    let summary = input.join().unwrap();
    let events = read_cursor(&dir).unwrap();
    println!("summary {summary:?}");
    let mut kinds = Vec::new();
    for e in &events {
        match e.event {
            CursorEvent::Image { id, x, y } => {
                let png = std::fs::read(dir.join(image_file(id))).unwrap();
                assert_eq!(&png[..4], b"\x89PNG");
                println!("image {id}: hotspot ({x}, {y}), {} bytes", png.len());
            }
            CursorEvent::Shape { kind, image } => {
                println!("{} µs: {kind:?} image {image:?}", e.at.micros());
                kinds.push(kind);
            }
            _ => {}
        }
    }
    kinds.retain(|k| matches!(k, CursorKind::Arrow | CursorKind::Text));
    kinds.dedup();
    assert!(kinds.len() >= 6, "arrow and I-beam alternate: {kinds:?}");
    let ids: Vec<u32> = events
        .iter()
        .filter_map(|e| match e.event {
            CursorEvent::Image { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, (0..ids.len() as u32).collect::<Vec<_>>());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Presses `chord` in order and releases it in reverse, in one `SendInput`.
fn chord(keys: &[u16]) {
    send(&chord_inputs(keys));
    pause(30);
}

#[test]
#[ignore = "types into the test window on monitor DEVCAST_TEST_MONITOR"]
fn typed_text_and_shortcuts_are_recorded() {
    const CTRL: u16 = 0x11;
    const SHIFT: u16 = 0x10;
    const F5: u8 = 0x74;
    let _back = PointerBack::save();
    let (target, x, y) = show_target();
    send(&at_point(x + 100, y + 150, MOUSEEVENTF_LEFTDOWN, 0));
    send(&at_point(x + 100, y + 150, MOUSEEVENTF_LEFTUP, 0));
    pause(200);
    // Typing anywhere else would type into the owner's windows.
    assert!(
        target.is_foreground(),
        "the test window did not get the keyboard"
    );
    let dir = folder("input-keys");
    let input = InputRecording::start().unwrap();
    input
        .begin(now().unwrap(), &dir, (0, 0), &PauseClock::default())
        .unwrap();
    for c in *b"ABC" {
        chord(&[u16::from(c)]);
    }
    chord(&[SHIFT, u16::from(b'A')]);
    chord(&[CTRL, SHIFT, u16::from(b'P')]);
    chord(&[u16::from(F5)]);
    // The user's last action is well before the stop.
    pause(200);
    let summary = input.join().unwrap();
    let keys: Vec<KeyEvent> = read_keys(&dir).unwrap().iter().map(|k| k.event).collect();
    println!(
        "typed into the window: {:?}; keys.bin: {keys:?}; {summary:?}",
        Target::typed()
    );
    let shape = common::synthetic::letter_shape;
    assert_eq!(shape(&Target::typed()), shape("abcA"));
    let with = |key: u8, ctrl, shift| KeyEvent {
        key: u16::from(key),
        modifiers: Modifiers {
            ctrl,
            shift,
            alt: false,
            win: false,
        },
    };
    let expected = [
        with(b'A', false, false),
        with(b'B', false, false),
        with(b'C', false, false),
        with(b'A', false, true),
        with(b'P', true, true),
        with(F5, false, false),
    ];
    assert_eq!(keys, expected);
    std::fs::remove_dir_all(&dir).unwrap();
}
