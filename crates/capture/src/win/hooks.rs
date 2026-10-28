//! Low-level input hooks on their own thread with a message loop (decision 009).
//!
//! Windows calls a low-level hook from the message loop of the thread that set it and removes
//! a hook that answers slower than `LowLevelHooksTimeout`, so the hook only stamps the event
//! with the host clock and hands it to the writer thread.

use std::cell::RefCell;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, GetPhysicalCursorPos, HC_ACTION, HHOOK, MSG,
    MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1,
};

use super::clock::now;
use crate::error::CaptureError;
use crate::input::{Button, CursorEvent, InputMessage, Timed};

thread_local! {
    /// Where the hooks of this thread send events; set while the hooks are installed.
    static EVENTS: RefCell<Option<Sender<InputMessage>>> = const { RefCell::new(None) };
}

/// The hook thread; dropping it removes the hooks.
pub struct Hooks {
    thread: u32,
    handle: Option<JoinHandle<()>>,
}

impl Hooks {
    /// Installs the hooks; from now on events go to `events`, the first being where the
    /// pointer is.
    pub fn start(events: Sender<InputMessage>) -> Result<Self, CaptureError> {
        let (ready, started) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("devcast-input-hooks".to_owned())
            .spawn(move || run(events, &ready))
            .map_err(|e| {
                CaptureError::platform("запустить запись мыши")(e.to_string())
            })?;
        let thread = started.recv().map_err(|_| {
            CaptureError::platform("запустить запись мыши")(
                "поток хуков упал".to_owned(),
            )
        })??;
        Ok(Self {
            thread,
            handle: Some(handle),
        })
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // SAFETY: posts a message to the hook thread, which is alive until joined below.
            // It fails only if that thread has no queue, which it creates before it reports
            // being ready.
            let _ = unsafe { PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0)) };
            let _ = handle.join();
        }
    }
}

fn run(events: Sender<InputMessage>, ready: &Sender<Result<u32, CaptureError>>) {
    let failed = CaptureError::platform("установить хук мыши");
    let mut message = MSG::default();
    // SAFETY: creates this thread's message queue so that WM_QUIT can be posted to it.
    let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
    EVENTS.set(Some(events));
    // SAFETY: a low-level hook with a procedure of this module; removed below on this thread.
    let hook = unsafe {
        GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(module.into()), 0)
        })
    };
    let hook = match hook {
        Ok(hook) => hook,
        Err(e) => {
            let _ = ready.send(Err(failed(e.to_string())));
            return;
        }
    };
    send(initial_position());
    // SAFETY: the id of this thread.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    // SAFETY: the message loop of this thread; it ends on WM_QUIT (0) or an error (−1).
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        // SAFETY: a message of this thread's queue.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    unhook(hook);
    EVENTS.set(None);
}

fn unhook(hook: HHOOK) {
    // SAFETY: the hook was set on this thread above and is removed once.
    if let Err(e) = unsafe { UnhookWindowsHookEx(hook) } {
        send(Err(CaptureError::platform("снять хук мыши")(
            e.to_string(),
        )));
    }
}

fn initial_position() -> Result<CursorEvent, CaptureError> {
    let mut point = POINT::default();
    // SAFETY: writes the pointer position into `point`.
    unsafe { GetPhysicalCursorPos(&mut point) }.map_err(|e| {
        CaptureError::platform("узнать положение курсора")(e.to_string())
    })?;
    Ok(CursorEvent::Move {
        x: point.x,
        y: point.y,
    })
}

/// Stamps `event` with the host clock now and sends it to the writer.
fn send(event: Result<CursorEvent, CaptureError>) {
    let message = match (now(), event) {
        (Ok(at), Ok(event)) => InputMessage::Mouse(Timed { at, event }),
        (Err(error), _) | (_, Err(error)) => InputMessage::Failed(error),
    };
    EVENTS.with_borrow(|events| {
        if let Some(events) = events {
            // The writer is gone only after it failed; its error is reported at the stop.
            let _ = events.send(message);
        }
    });
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for WH_MOUSE_LL with HC_ACTION, lparam points to an MSLLHOOKSTRUCT that the
        // system owns for the duration of the call.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if let Some(event) = mouse_event(wparam.0 as u32, info) {
            send(Ok(event));
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
    let delta = i32::from(high as i16);
    Some(match message {
        WM_MOUSEMOVE => CursorEvent::Move { x, y },
        WM_LBUTTONDOWN => CursorEvent::Down {
            button: Button::Left,
            x,
            y,
        },
        WM_LBUTTONUP => CursorEvent::Up {
            button: Button::Left,
            x,
            y,
        },
        WM_RBUTTONDOWN => CursorEvent::Down {
            button: Button::Right,
            x,
            y,
        },
        WM_RBUTTONUP => CursorEvent::Up {
            button: Button::Right,
            x,
            y,
        },
        WM_MBUTTONDOWN => CursorEvent::Down {
            button: Button::Middle,
            x,
            y,
        },
        WM_MBUTTONUP => CursorEvent::Up {
            button: Button::Middle,
            x,
            y,
        },
        WM_XBUTTONDOWN => CursorEvent::Down { button: side, x, y },
        WM_XBUTTONUP => CursorEvent::Up { button: side, x, y },
        WM_MOUSEWHEEL => CursorEvent::Wheel { x, y, delta },
        WM_MOUSEHWHEEL => CursorEvent::HWheel { x, y, delta },
        _ => return None,
    })
}
