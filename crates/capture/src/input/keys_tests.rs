//! Tests of the privacy filter and of `keys.bin`.

use super::*;
use crate::input::file::RecordWriter;

const CTRL: u16 = 0xA2;
const SHIFT: u16 = 0xA0;
const ALT: u16 = 0xA4;
const WIN: u16 = 0x5B;
const F5: u16 = 0x74;
const ESC: u16 = 0x1B;

/// Presses `chord` in order and releases it in reverse, through the filter.
fn press(filter: &mut KeyFilter, chord: &[u16]) -> Vec<KeyEvent> {
    let downs = chord.iter().filter_map(|&k| filter.key(stroke(k, true)));
    let mut kept: Vec<KeyEvent> = downs.collect();
    kept.extend(
        chord
            .iter()
            .rev()
            .filter_map(|&k| filter.key(stroke(k, false))),
    );
    kept
}

fn stroke(key: u16, down: bool) -> Stroke {
    Stroke {
        key,
        scan: 0,
        extended: false,
        down,
    }
}

fn letter(c: char) -> u16 {
    c.to_ascii_uppercase() as u16
}

#[test]
fn typed_text_is_dropped_and_shortcuts_are_kept() {
    let mut filter = KeyFilter::default();
    for c in "abc 123".chars() {
        assert_eq!(press(&mut filter, &[letter(c)]), [], "{c}");
    }
    assert_eq!(press(&mut filter, &[SHIFT, letter('a')]), [], "Shift+A");
    assert_eq!(press(&mut filter, &[SHIFT]), []);
    let shortcut = |key, ctrl, shift, alt, win| KeyEvent {
        key,
        modifiers: Modifiers {
            ctrl,
            shift,
            alt,
            win,
        },
    };
    let cases = [
        (
            vec![CTRL, SHIFT, letter('p')],
            shortcut(letter('p'), true, true, false, false),
        ),
        (vec![F5], shortcut(F5, false, false, false, false)),
        (vec![ESC], shortcut(ESC, false, false, false, false)),
        (vec![SHIFT, 0x09], shortcut(0x09, false, true, false, false)),
        (vec![ALT, 0x09], shortcut(0x09, false, false, true, false)),
        (
            vec![WIN, letter('d')],
            shortcut(letter('d'), false, false, false, true),
        ),
        (
            vec![0x11, letter('z')],
            shortcut(letter('z'), true, false, false, false),
        ),
    ];
    for (chord, expected) in cases {
        assert_eq!(press(&mut filter, &chord), [expected], "{chord:x?}");
    }
    // After the shortcuts every modifier is up again: plain typing is dropped.
    assert_eq!(press(&mut filter, &[letter('q')]), []);
}

#[test]
fn special_keys_are_exactly_the_list() {
    let mut filter = KeyFilter::default();
    let kept: Vec<u16> = (1..=0xFE)
        .filter(|&key| {
            let kept = filter.key(stroke(key, true)).is_some();
            filter.key(stroke(key, false));
            kept
        })
        .collect();
    let mut expected = vec![0x08, 0x09, 0x0D, 0x1B];
    expected.extend(0x21..=0x28);
    expected.extend(0x2C..=0x2E);
    expected.extend(0x70..=0x87);
    assert_eq!(kept, expected);
}

#[test]
fn records_round_trip_through_the_file() {
    let event = Timed {
        at: HostTime::from_micros(-5),
        event: KeyEvent {
            key: 0x50,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                alt: false,
                win: true,
            },
        },
    };
    let dir = tempfile::tempdir().unwrap();
    let mut writer = RecordWriter::create(&dir.path().join(KEYS_FILE)).unwrap();
    writer.write(&event).unwrap();
    writer.flush().unwrap();
    drop(writer);
    let bytes = std::fs::read(dir.path().join(KEYS_FILE)).unwrap();
    assert_eq!(&bytes[..8], b"DCKY\x01\x00\x0C\x00");
    assert_eq!(&bytes[16..], [0x50, 0, 0b1011, 0]);
    assert_eq!(read_keys(dir.path()).unwrap(), [event]);
    let mut broken = bytes;
    broken[18] = 0x10;
    std::fs::write(dir.path().join(KEYS_FILE), broken).unwrap();
    assert!(read_keys(dir.path()).is_err());
}
