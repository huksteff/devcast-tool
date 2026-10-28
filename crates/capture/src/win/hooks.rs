//! Low-level input hooks on their own thread with a message loop (decision 009).
//!
//! Windows calls a low-level hook from the message loop of the thread that set it and removes
//! a hook that answers slower than `LowLevelHooksTimeout`, so the hook only stamps the event
//! with the host clock and hands it to the writer thread. Cursor shape changes come to the
//! same loop as accessibility events of the cursor object (`OBJID_CURSOR`: shown, hidden,
//! «name» changed = new shape); the picture of a cursor handle is read the first time it is
//! seen, while the handle is surely alive, and encoded by the writer.

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_HIDE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, GetMessageW,
    GetPhysicalCursorPos, HHOOK, MSG, OBJID_CURSOR, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT,
    WM_QUIT,
};

use super::clock::now;
use super::cursor::{SystemCursors, current, picture};
use super::mouse::mouse_proc;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::input::{CursorEvent, InputMessage, SeenShape, Timed};

thread_local! {
    /// What the hooks of this thread need; set while the hooks are installed.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

struct State {
    events: Sender<InputMessage>,
    cursors: SystemCursors,
    /// Cursor handles whose picture was sent.
    seen: HashSet<isize>,
    /// The last cursor handle sent and whether it was shown.
    last: Option<(isize, bool)>,
}

impl State {
    /// The cursor shape at `at`, if it changed since the last look.
    fn look(&mut self, at: HostTime) -> Result<Option<InputMessage>, CaptureError> {
        let (handle, visible) = current()?;
        if self.last == Some((handle, visible)) {
            return Ok(None);
        }
        self.last = Some((handle, visible));
        let image = match visible && self.seen.insert(handle) {
            true => Some(picture(handle)?),
            false => None,
        };
        let event = SeenShape {
            handle,
            kind: self.cursors.kind(handle),
            visible,
            image,
        };
        Ok(Some(InputMessage::Shape(Timed { at, event })))
    }
}

/// The hook thread; dropping it removes the hooks.
pub struct Hooks {
    thread: u32,
    handle: Option<JoinHandle<()>>,
}

impl Hooks {
    /// Installs the hooks; from now on events go to `events`, the first being where the
    /// pointer is and what the cursor is.
    pub fn start(events: Sender<InputMessage>) -> Result<Self, CaptureError> {
        let failed = || CaptureError::platform("запустить запись мыши");
        let (ready, started) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("devcast-input-hooks".to_owned())
            .spawn(move || run(events, &ready))
            .map_err(|e| failed()(e.to_string()))?;
        let thread = started
            .recv()
            .map_err(|_| failed()("поток хуков упал".to_owned()))??;
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
    let mut message = MSG::default();
    // SAFETY: creates this thread's message queue so that WM_QUIT can be posted to it.
    let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
    let hooks = match install(events) {
        Ok(hooks) => hooks,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    send_mouse(initial_position());
    look_at_cursor();
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
    unhook(hooks);
    STATE.set(None);
}

/// The mouse hook and the two cursor event hooks (shown/hidden, shape).
type Installed = (HHOOK, [HWINEVENTHOOK; 2]);

fn install(events: Sender<InputMessage>) -> Result<Installed, CaptureError> {
    STATE.set(Some(State {
        events,
        cursors: SystemCursors::load()?,
        seen: HashSet::new(),
        last: None,
    }));
    // SAFETY: a low-level hook with a procedure of this module; removed by `unhook` on this
    // thread.
    let mouse = unsafe {
        GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(module.into()), 0)
        })
    }
    .map_err(|e| CaptureError::platform("установить хук мыши")(e.to_string()))?;
    let ranges = [
        (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE),
        (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
    ];
    // SAFETY: out-of-context event hooks of all processes, delivered to this thread's message
    // loop; removed by `unhook` on this thread.
    let cursor = ranges.map(|(first, last)| unsafe {
        SetWinEventHook(
            first,
            last,
            None,
            Some(cursor_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    });
    if cursor.iter().any(|hook| hook.is_invalid()) {
        unhook((mouse, cursor));
        return Err(CaptureError::platform("следить за курсором")(
            "SetWinEventHook отказал".to_owned(),
        ));
    }
    Ok((mouse, cursor))
}

fn unhook((mouse, cursor): Installed) {
    for hook in cursor.into_iter().filter(|hook| !hook.is_invalid()) {
        // SAFETY: an event hook set on this thread, removed once. It fails only for a hook
        // that is already gone, which leaves nothing to remove.
        let _ = unsafe { UnhookWinEvent(hook) };
    }
    // SAFETY: the hook was set on this thread and is removed once.
    if let Err(e) = unsafe { UnhookWindowsHookEx(mouse) } {
        post(Err(CaptureError::platform("снять хук мыши")(
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

/// Sends `message` (or the error that took its place) to the writer.
fn post(message: Result<InputMessage, CaptureError>) {
    let message = message.unwrap_or_else(InputMessage::Failed);
    STATE.with_borrow(|state| {
        if let Some(state) = state {
            // The writer is gone only after it failed; its error is reported at the stop.
            let _ = state.events.send(message);
        }
    });
}

/// Stamps `event` with the host clock now and sends it to the writer.
pub fn send_mouse(event: Result<CursorEvent, CaptureError>) {
    post(now().and_then(|at| Ok(InputMessage::Mouse(Timed { at, event: event? }))));
}

/// Sends the cursor shape if it changed.
fn look_at_cursor() {
    let looked = now().and_then(|at| {
        STATE.with_borrow_mut(|state| match state {
            Some(state) => state.look(at),
            None => Ok(None),
        })
    });
    if let Some(message) = looked.transpose() {
        post(message);
    }
}

unsafe extern "system" fn cursor_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _window: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if object == OBJID_CURSOR.0 {
        look_at_cursor();
    }
}
