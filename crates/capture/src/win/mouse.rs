//! The low-level mouse hook procedure: a message and its `MSLLHOOKSTRUCT` → a mouse event.

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, MSLLHOOKSTRUCT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1,
};

use super::hooks::send_mouse;
use crate::input::{Button, CursorEvent};

pub unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for WH_MOUSE_LL with HC_ACTION, lparam points to an MSLLHOOKSTRUCT that the
        // system owns for the duration of the call.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if let Some(event) = mouse_event(wparam.0 as u32, info) {
            send_mouse(Ok(event));
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// The event of a low-level mouse message; `pt` is in physical desktop pixels.
fn mouse_event(message: u32, info: &MSLLHOOKSTRUCT) -> Option<CursorEvent> {
    let (x, y) = (info.pt.x, info.pt.y);
    let high = (info.mouseData >> 16) as u16;
    let side = if high == XBUTTON1 {
        Button::X1
    } else {
        Button::X2
    };
    let (button, down) = match message {
        WM_MOUSEMOVE => return Some(CursorEvent::Move { x, y }),
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let delta = i32::from(high as i16);
            return Some(match message {
                WM_MOUSEWHEEL => CursorEvent::Wheel { x, y, delta },
                _ => CursorEvent::HWheel { x, y, delta },
            });
        }
        WM_LBUTTONDOWN => (Button::Left, true),
        WM_LBUTTONUP => (Button::Left, false),
        WM_RBUTTONDOWN => (Button::Right, true),
        WM_RBUTTONUP => (Button::Right, false),
        WM_MBUTTONDOWN => (Button::Middle, true),
        WM_MBUTTONUP => (Button::Middle, false),
        WM_XBUTTONDOWN => (side, true),
        WM_XBUTTONUP => (side, false),
        _ => return None,
    };
    Some(match down {
        true => CursorEvent::Down { button, x, y },
        false => CursorEvent::Up { button, x, y },
    })
}
