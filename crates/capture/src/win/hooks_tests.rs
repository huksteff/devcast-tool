//! The keyboard hook procedure on synthetic hook events (no real input): typed text never
//! leaves the hook, shortcuts reach `keys.bin`.

use std::sync::mpsc;

use windows::Win32::UI::WindowsAndMessaging::{
    HC_ACTION, KBDLLHOOKSTRUCT, KBDLLHOOKSTRUCT_FLAGS, LLKHF_EXTENDED, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use super::super::keys::keyboard_proc;
use super::*;
use crate::input::{KEYS_FILE, KeyEvent, Modifiers, read_keys, write};

const CTRL: u16 = 0xA2;
const SHIFT: u16 = 0xA0;
const ALT: u16 = 0xA4;
const TAB: u16 = 0x09;
const F5: u16 = 0x74;
const RALT: u16 = 0xA5;
const DELETE: u16 = 0x2E;

/// Calls the hook procedure as Windows would for key `key` going down or up.
fn hook(key: u16, down: bool, alt: bool) {
    hook_scan(key, 0, false, down, alt);
}

/// As [`hook`], with the scan code and the extended-key flag.
fn hook_scan(key: u16, scan: u32, extended: bool, down: bool, alt: bool) {
    let info = KBDLLHOOKSTRUCT {
        vkCode: u32::from(key),
        scanCode: scan,
        flags: if extended {
            LLKHF_EXTENDED
        } else {
            KBDLLHOOKSTRUCT_FLAGS(0)
        },
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

/// Installs the state of a hook thread on this thread for the run of `strokes`, and returns
/// what the hooks queued.
fn queued(strokes: impl FnOnce()) -> Vec<InputMessage> {
    let (events, hooked) = mpsc::channel();
    STATE.set(Some(State {
        events,
        cursors: SystemCursors::load().unwrap(),
        seen: HashSet::new(),
        last: None,
        keys: KeyFilter::default(),
    }));
    strokes();
    STATE.set(None);
    hooked.iter().collect()
}

fn keys_of(queued: &[InputMessage]) -> Vec<KeyEvent> {
    queued
        .iter()
        .filter_map(|m| match m {
            InputMessage::Key(key) => Some(key.event),
            _ => None,
        })
        .collect()
}

fn with(key: u16, ctrl: bool, shift: bool, alt: bool) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers {
            ctrl,
            shift,
            alt,
            win: false,
        },
    }
}

/// AltGr as Windows reports it: a left Ctrl with scan code 0x21D, then the right Alt.
fn altgr(key: u8) {
    hook_scan(CTRL, 0x21D, false, true, false);
    hook_scan(RALT, 0x38, true, true, true);
    hook(u16::from(key), true, true);
    hook(u16::from(key), false, true);
    hook_scan(CTRL, 0x21D, false, false, true);
    hook_scan(RALT, 0x38, true, false, true);
}

#[test]
fn altgr_characters_and_alt_codes_are_text() {
    let queued = queued(|| {
        for key in *b"QEL" {
            altgr(key);
        }
        // Alt + Numpad 6, 5 types «A»; without Num Lock the Numpad sends navigation codes
        // without the extended flag.
        press(&[ALT, 0x66, 0x65]);
        hook(ALT, true, true);
        for key in [0x27, 0x0C] {
            hook_scan(key, 0, false, true, true);
            hook_scan(key, 0, false, false, true);
        }
        hook(ALT, false, true);
        // Real shortcuts with the left keys and the grey arrow.
        hook_scan(CTRL, 0x1D, false, true, false);
        hook_scan(ALT, 0x38, false, true, true);
        hook_scan(DELETE, 0x53, true, true, true);
        hook_scan(DELETE, 0x53, true, false, true);
        hook_scan(ALT, 0x38, false, false, true);
        hook_scan(CTRL, 0x1D, false, false, false);
        press(&[ALT, TAB]);
        hook(ALT, true, true);
        hook_scan(0x25, 0x4B, true, true, true);
        hook_scan(0x25, 0x4B, true, false, true);
        hook(ALT, false, true);
    });
    assert_eq!(queued.len(), 3, "only shortcuts are queued");
    let expected = [
        with(DELETE, true, false, true),
        with(TAB, false, false, true),
        with(0x25, false, false, true),
    ];
    assert_eq!(keys_of(&queued), expected);
}

#[test]
fn typed_text_never_leaves_the_hook_and_shortcuts_reach_the_file() {
    let queued = queued(|| {
        for key in "ABC".encode_utf16() {
            press(&[key]);
        }
        press(&[SHIFT, u16::from(b'A')]);
        press(&[u16::from(b'1'), 0x20]);
        press(&[CTRL, SHIFT, u16::from(b'P')]);
        press(&[ALT, TAB]);
        press(&[F5]);
    });
    let keys = keys_of(&queued);
    assert_eq!(queued.len(), keys.len(), "only shortcuts are queued");
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
