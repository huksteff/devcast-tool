//! Tests of the input writer.

use std::sync::mpsc;

use super::*;
use crate::input::cursor::{Button, CursorKind, read_cursor};

fn fake_png(image: &CursorImage) -> Result<Vec<u8>, CaptureError> {
    Ok(image.rgba.clone())
}

fn picture(shade: u8) -> CursorImage {
    CursorImage {
        width: 1,
        height: 1,
        hotspot: (0, 0),
        rgba: vec![shade, shade, shade, 255],
    }
}

fn shape(micros: i64, handle: isize, kind: CursorKind, image: Option<CursorImage>) -> InputMessage {
    InputMessage::Shape(Timed {
        at: HostTime::from_micros(micros),
        event: SeenShape {
            handle,
            kind,
            visible: handle != 0,
            image,
        },
    })
}

fn begin(micros: i64, dir: &std::path::Path, clock: &PauseClock) -> InputMessage {
    InputMessage::Begin {
        start: HostTime::from_micros(micros),
        dir: dir.to_path_buf(),
        origin: (0, 0),
        clock: clock.clone(),
    }
}

fn at(micros: i64, event: CursorEvent) -> InputMessage {
    InputMessage::Mouse(Timed {
        at: HostTime::from_micros(micros),
        event,
    })
}

#[test]
fn starts_at_the_last_position_and_skips_still_moves() {
    let dir = tempfile::tempdir().unwrap();
    let (send, receive) = mpsc::channel();
    let (x, y, button) = (2, 2, Button::Left);
    let still = CursorEvent::Move { x, y };
    let down = CursorEvent::Down { button, x, y };
    let up = CursorEvent::Up { button, x, y };
    let wheel = CursorEvent::Wheel { x, y, delta: 120 };
    let moved = CursorEvent::Move { x: 4, y };
    for message in [
        at(10, CursorEvent::Move { x: 1, y: 1 }),
        at(20, still),
        begin(100, dir.path(), &PauseClock::default()),
        at(90, CursorEvent::Move { x: 3, y: 3 }),
        at(110, still),
        at(120, down),
        at(130, still),
        at(140, up),
        at(150, wheel),
        at(160, moved),
    ] {
        send.send(message).unwrap();
    }
    drop(send);
    let summary = write(&receive, fake_png).unwrap();
    let expected = [
        (100, still),
        (120, down),
        (140, up),
        (150, wheel),
        (160, moved),
    ]
    .map(|(micros, event)| Timed {
        at: HostTime::from_micros(micros),
        event,
    });
    assert_eq!(read_cursor(dir.path()).unwrap(), expected);
    let counts = InputSummary {
        moves: 2,
        downs: 1,
        ups: 1,
        wheels: 1,
        ..InputSummary::default()
    };
    assert_eq!(summary, counts);
}

#[test]
fn a_hook_failure_ends_the_writer_with_its_error() {
    let (send, receive) = mpsc::channel();
    let error =
        CaptureError::platform("прочитать системные часы")("нет".to_owned());
    send.send(InputMessage::Failed(error)).unwrap();
    assert!(matches!(
        write(&receive, fake_png),
        Err(CaptureError::Platform { .. })
    ));
}

#[test]
fn shapes_start_with_the_recording_and_share_equal_pictures() {
    let dir = tempfile::tempdir().unwrap();
    let (send, receive) = mpsc::channel();
    let (arrow, text, custom) = (CursorKind::Arrow, CursorKind::Text, CursorKind::Custom);
    for message in [
        shape(10, 1, arrow, Some(picture(0))),
        shape(20, 2, text, Some(picture(9))),
        begin(100, dir.path(), &PauseClock::default()),
        shape(110, 3, custom, Some(picture(0))),
        shape(120, 1, arrow, None),
        shape(130, 0, arrow, None),
        shape(140, 2, text, None),
    ] {
        send.send(message).unwrap();
    }
    drop(send);
    let summary = write(&receive, fake_png).unwrap();
    let image = |id| CursorEvent::Image { id, x: 0, y: 0 };
    let show = |kind, id| CursorEvent::Shape { kind, image: id };
    let expected = [
        (100, image(0)),
        (100, show(text, Some(0))),
        (110, image(1)),
        (110, show(custom, Some(1))),
        (120, show(arrow, Some(1))),
        (130, show(arrow, None)),
        (140, show(text, Some(0))),
    ]
    .map(|(micros, event)| Timed {
        at: HostTime::from_micros(micros),
        event,
    });
    assert_eq!(read_cursor(dir.path()).unwrap(), expected);
    assert_eq!((summary.shapes, summary.images), (5, 2));
    let saved = std::fs::read(dir.path().join(image_file(1))).unwrap();
    assert_eq!(saved, picture(0).rgba);
    assert!(!dir.path().join(image_file(2)).exists());
}

#[test]
fn pauses_are_cut_out_and_the_pointer_catches_up_on_resume() {
    let dir = tempfile::tempdir().unwrap();
    let (send, receive) = mpsc::channel();
    let clock = PauseClock::default();
    let micros = HostTime::from_micros;
    let (x, y, button) = (5, 5, Button::Left);
    let key = |at| {
        InputMessage::Key(Timed {
            at: micros(at),
            event: KeyEvent {
                key: 0x1B,
                modifiers: crate::input::Modifiers::default(),
            },
        })
    };
    send.send(begin(100, dir.path(), &clock)).unwrap();
    send.send(at(150, CursorEvent::Move { x, y })).unwrap();
    clock.pause(micros(200)).unwrap();
    send.send(at(250, CursorEvent::Down { button, x, y }))
        .unwrap();
    send.send(at(260, CursorEvent::Move { x: 9, y: 9 }))
        .unwrap();
    send.send(key(270)).unwrap();
    clock.resume(micros(1200)).unwrap();
    send.send(InputMessage::Resumed(micros(1200))).unwrap();
    send.send(at(1300, CursorEvent::Up { button, x: 9, y: 9 }))
        .unwrap();
    send.send(key(1310)).unwrap();
    drop(send);
    write(&receive, fake_png).unwrap();
    let expected = [
        (150, CursorEvent::Move { x, y }),
        (200, CursorEvent::Move { x: 9, y: 9 }),
        (300, CursorEvent::Up { button, x: 9, y: 9 }),
    ]
    .map(|(micros, event)| Timed {
        at: HostTime::from_micros(micros),
        event,
    });
    assert_eq!(read_cursor(dir.path()).unwrap(), expected);
    let keys = crate::input::read_keys(dir.path()).unwrap();
    assert_eq!(keys.iter().map(|k| k.at).collect::<Vec<_>>(), [micros(310)]);
}

#[test]
fn positions_are_relative_to_the_recorded_picture() {
    let dir = tempfile::tempdir().unwrap();
    let (send, receive) = mpsc::channel();
    let button = Button::Right;
    for message in [
        at(10, CursorEvent::Move { x: -1900, y: 300 }),
        InputMessage::Begin {
            start: HostTime::from_micros(100),
            dir: dir.path().to_path_buf(),
            origin: (-1920, 11),
            clock: PauseClock::default(),
        },
        at(
            110,
            CursorEvent::Down {
                button,
                x: -1920,
                y: 11,
            },
        ),
        at(
            120,
            CursorEvent::Wheel {
                x: 5,
                y: 0,
                delta: -120,
            },
        ),
        shape(130, 1, CursorKind::Arrow, Some(picture(0))),
    ] {
        send.send(message).unwrap();
    }
    drop(send);
    write(&receive, fake_png).unwrap();
    let events: Vec<CursorEvent> = read_cursor(dir.path())
        .unwrap()
        .iter()
        .map(|e| e.event)
        .collect();
    let shape = CursorEvent::Shape {
        kind: CursorKind::Arrow,
        image: Some(0),
    };
    assert_eq!(
        events,
        [
            CursorEvent::Move { x: 20, y: 289 },
            CursorEvent::Down { button, x: 0, y: 0 },
            CursorEvent::Wheel {
                x: 1925,
                y: -11,
                delta: -120
            },
            CursorEvent::Image { id: 0, x: 0, y: 0 },
            shape,
        ]
    );
}
