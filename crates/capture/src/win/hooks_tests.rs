//! The keyboard hook procedure on synthetic hook events (no real input): typed text never
//! leaves the hook, shortcuts reach `keys.bin`.

use std::sync::mpsc;

use windows::Win32::UI::WindowsAndMessaging::{
    HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use super::super::keys::keyboard_proc;
use super::*;
use crate::input::{KEYS_FILE, KeyEvent, Modifiers, read_keys, write};

const CTRL: u16 = 0xA2;
const SHIFT: u16 = 0xA0;
const ALT: u16 = 0xA4;
const TAB: u16 = 0x09;
const F5: u16 = 0x74;

/// Calls the hook procedure as Windows would for key `key` going down or up.
fn hook(key: u16, down: bool, alt: bool) {
    let info = KBDLLHOOKSTRUCT {
        vkCode: u32::from(key),
        ..Default::default()
    };
    let message = match (down, alt) {
        (true, false) => WM_KEYDOWN,
        (false, false) => WM_KEYUP,
        (true, true) => WM_SYSKEYDOWN,
        (false, true) => WM_SYSKEYUP,
    };
    // SAFETY: the procedure reads `info` during the call only.
    unsafe {
        keyboard_proc(
            HC_ACTION as i32,
            WPARAM(message as usize),
            LPARAM((&raw const info) as isize),
        )
    };
}

/// Presses `chord` in order and releases it in reverse.
fn press(chord: &[u16]) {
    let alt = chord.contains(&ALT);
    for &key in chord {
        hook(key, true, alt);
    }
    for &key in chord.iter().rev() {
        hook(key, false, alt);
    }
}

#[test]
fn typed_text_never_leaves_the_hook_and_shortcuts_reach_the_file() {
    let (events, hooked) = mpsc::channel();
    STATE.set(Some(State {
        events,
        cursors: SystemCursors::load().unwrap(),
        seen: HashSet::new(),
        last: None,
        keys: KeyFilter::default(),
    }));
    for key in "ABC".encode_utf16() {
        press(&[key]);
    }
    press(&[SHIFT, u16::from(b'A')]);
    press(&[u16::from(b'1'), 0x20]);
    press(&[CTRL, SHIFT, u16::from(b'P')]);
    press(&[ALT, TAB]);
    press(&[F5]);
    STATE.set(None);
    let queued: Vec<InputMessage> = hooked.iter().collect();
    let keys: Vec<KeyEvent> = queued
        .iter()
        .filter_map(|m| match m {
            InputMessage::Key(key) => Some(key.event),
            _ => None,
        })
        .collect();
    assert_eq!(queued.len(), keys.len(), "only shortcuts are queued");
    let with = |key, ctrl, shift, alt| KeyEvent {
        key,
        modifiers: Modifiers {
            ctrl,
            shift,
            alt,
            win: false,
        },
    };
    let expected = [
        with(u16::from(b'P'), true, true, false),
        with(TAB, false, false, true),
        with(F5, false, false, false),
    ];
    assert_eq!(keys, expected);

    let dir = tempfile::tempdir().unwrap();
    let (send, receive) = mpsc::channel();
    let begin = InputMessage::Begin(HostTime::from_micros(0), dir.path().to_path_buf());
    for message in std::iter::once(begin).chain(queued) {
        send.send(message).unwrap();
    }
    drop(send);
    let summary = write(&receive, |_| Ok(Vec::new())).unwrap();
    assert_eq!(summary.keys, 3);
    let written: Vec<KeyEvent> = read_keys(dir.path())
        .unwrap()
        .iter()
        .map(|k| k.event)
        .collect();
    assert_eq!(written, expected);
    let bytes = std::fs::read(dir.path().join(KEYS_FILE)).unwrap();
    let typed = bytes[8..]
        .chunks(12)
        .filter(|r| matches!(r[8], b'A'..=b'C') && r[10] & 0b1101 == 0);
    assert_eq!(typed.count(), 0, "abc or Shift+A in keys.bin");
}
