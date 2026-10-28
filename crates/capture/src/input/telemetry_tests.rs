use super::*;
use crate::input::file::RecordWriter;
use crate::input::{CURSOR_FILE, KEYS_FILE, Modifiers, Timed};
use crate::manifest::Track;

const START: i64 = 5_000_000;

fn ms(millis: i64) -> SourceTime {
    SourceTime::from_micros(millis * 1000)
}

fn down(millis: i64, button: Button, x: i32, y: i32) -> Timed<CursorEvent> {
    Timed {
        at: HostTime::from_micros(START + millis * 1000),
        event: CursorEvent::Down { button, x, y },
    }
}

/// A recording folder whose video starts at host time `START`, with these events.
fn folder(cursor: &[Timed<CursorEvent>], keys: &[Timed<KeyEvent>]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let video = Track::new(VIDEO_TRACK, 60, HostTime::from_micros(START));
    let sound = Track::new("mic", 48_000, HostTime::from_micros(START - 7));
    Manifest::new(vec![sound, video]).write(dir.path()).unwrap();
    let mut writer = RecordWriter::create(&dir.path().join(CURSOR_FILE)).unwrap();
    for event in cursor {
        writer.write(event).unwrap();
    }
    writer.flush().unwrap();
    let mut writer = RecordWriter::create(&dir.path().join(KEYS_FILE)).unwrap();
    for event in keys {
        writer.write(event).unwrap();
    }
    writer.flush().unwrap();
    dir
}

#[test]
fn events_are_on_the_source_time_of_the_video() {
    let key = KeyEvent {
        key: 0x74,
        modifiers: Modifiers::default(),
    };
    let start = Timed {
        at: HostTime::from_micros(START),
        event: CursorEvent::Move { x: 10, y: 20 },
    };
    let dir = folder(
        &[start, down(1500, Button::Left, 11, 21)],
        &[Timed {
            at: HostTime::from_micros(START + 2_000_000),
            event: key,
        }],
    );
    let telemetry = Telemetry::read(dir.path()).unwrap();
    assert_eq!(
        telemetry.cursor[0],
        SourceEvent {
            at: SourceTime::ZERO,
            event: CursorEvent::Move { x: 10, y: 20 }
        }
    );
    assert_eq!(telemetry.cursor[1].at, ms(1500));
    assert_eq!(
        telemetry.keys,
        [SourceEvent {
            at: ms(2000),
            event: key
        }]
    );
}

#[test]
fn double_clicks_follow_the_windows_rule() {
    let dir = folder(
        &[
            down(0, Button::Left, 100, 100),
            down(400, Button::Left, 102, 98),
            down(700, Button::Left, 102, 98),
            down(2000, Button::Left, 500, 500),
            down(2300, Button::Right, 500, 500),
            down(3000, Button::Left, 500, 500),
            down(3200, Button::Left, 503, 500),
            down(5000, Button::Left, 0, 0),
            down(5501, Button::Left, 0, 0),
        ],
        &[],
    );
    let clicks = Telemetry::read(dir.path()).unwrap().clicks();
    let doubles: Vec<(i64, bool)> = clicks
        .iter()
        .map(|c| (c.at.micros() / 1000, c.double))
        .collect();
    assert_eq!(
        doubles,
        [
            (0, false),
            (400, true),
            (700, false),
            (2000, false),
            (2300, false),
            (3000, false),
            (3200, false),
            (5000, false),
            (5501, false),
        ],
        "a third press starts anew; other button, 3 px away or 501 ms later is not double"
    );
    assert_eq!(
        (clicks[1].x, clicks[1].y, clicks[1].button),
        (102, 98, Button::Left)
    );
}

#[test]
fn a_folder_without_manifest_or_video_is_an_error() {
    let empty = tempfile::tempdir().unwrap();
    assert!(Telemetry::read(empty.path()).is_err());
    let dir = folder(&[], &[]);
    let sound = Track::new("mic", 48_000, HostTime::from_micros(0));
    Manifest::new(vec![sound]).write(dir.path()).unwrap();
    let error = Telemetry::read(dir.path()).unwrap_err().to_string();
    assert!(error.contains("нет видео-дорожки"), "{error}");
}
