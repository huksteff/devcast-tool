//! Tests of the key filter and of `keys.bin`.

use super::*;
use crate::input::file::RecordWriter;

const CTRL: u16 = 0xA2;
const SHIFT: u16 = 0xA0;
const ALT: u16 = 0xA4;
const WIN: u16 = 0x5B;
const F5: u16 = 0x74;

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
    Stroke { key, scan: 0, down }
}

fn letter(c: char) -> u16 {
    c.to_ascii_uppercase() as u16
}

fn pressed(key: u16, ctrl: bool, shift: bool, alt: bool, win: bool) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers {
            ctrl,
            shift,
            alt,
            win,
        },
    }
}

#[test]
fn every_key_is_kept_with_its_modifiers() {
    let mut filter = KeyFilter::default();
    for c in "abc 123".chars() {
        let key = letter(c);
        assert_eq!(
            press(&mut filter, &[key]),
            [pressed(key, false, false, false, false)],
            "{c}"
        );
    }
    let cases = [
        (
            vec![SHIFT, letter('a')],
            pressed(letter('a'), false, true, false, false),
        ),
        (
            vec![CTRL, SHIFT, letter('p')],
            pressed(letter('p'), true, true, false, false),
        ),
        (vec![F5], pressed(F5, false, false, false, false)),
        (vec![ALT, 0x09], pressed(0x09, false, false, true, false)),
        (
            vec![WIN, letter('d')],
            pressed(letter('d'), false, false, false, true),
        ),
        (
            vec![0x11, letter('z')],
            pressed(letter('z'), true, false, false, false),
        ),
    ];
    for (chord, expected) in cases {
        assert_eq!(press(&mut filter, &chord), [expected], "{chord:x?}");
    }
    // After the chords every modifier is up again.
    assert_eq!(
        press(&mut filter, &[letter('q')]),
        [pressed(letter('q'), false, false, false, false)]
    );
}

#[test]
fn a_modifier_tapped_alone_is_a_press_on_its_release() {
    let mut filter = KeyFilter::default();
    assert_eq!(filter.key(stroke(CTRL, true)), None);
    // Auto-repeat while held.
    assert_eq!(filter.key(stroke(CTRL, true)), None);
    assert_eq!(
        filter.key(stroke(CTRL, false)),
        Some(pressed(CTRL, false, false, false, false))
    );
    // Ctrl, then Shift, both released: one press, Ctrl+Shift.
    assert_eq!(
        press(&mut filter, &[CTRL, SHIFT]),
        [pressed(SHIFT, true, false, false, false)]
    );
    assert_eq!(
        pressed(SHIFT, true, false, false, false).name(),
        "Ctrl+Shift"
    );
    // A modifier of a chord is not a press of its own.
    assert_eq!(press(&mut filter, &[CTRL, letter('c')]).len(), 1);
    assert_eq!(
        press(&mut filter, &[WIN]),
        [pressed(WIN, false, false, false, false)]
    );
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
