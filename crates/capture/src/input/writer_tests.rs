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
        InputMessage::Begin(HostTime::from_micros(100), dir.path().to_path_buf()),
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
        InputMessage::Begin(HostTime::from_micros(100), dir.path().to_path_buf()),
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
