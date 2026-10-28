//! A window of the test process on the test monitor: the only place synthetic input goes.
//! The cursor is the arrow over its left half and the I-beam over its right half.

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos,
    GetMessageW, HTCLIENT, IDC_ARROW, IDC_IBEAM, LoadCursorW, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, SetCursor, TranslateMessage, WM_CLOSE, WM_DESTROY, WM_SETCURSOR, WNDCLASSW,
    WS_BORDER, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::w;

/// Shows the window until dropped.
pub struct Target {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl Target {
    /// A `width`×`height` window at desktop pixel (`x`, `y`), above other windows.
    pub fn show(x: i32, y: i32, width: i32, height: i32) -> Self {
        let (created, window) = mpsc::channel();
        let thread = std::thread::spawn(move || run(x, y, width, height, &created));
        Self {
            hwnd: window.recv().unwrap(),
            thread: Some(thread),
        }
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        // SAFETY: our window; its thread destroys it on WM_CLOSE and then ends.
        let _ = unsafe {
            PostMessageW(
                Some(HWND(self.hwnd as *mut _)),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            )
        };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_SETCURSOR if (lparam.0 & 0xFFFF) as u32 == HTCLIENT => {
            // SAFETY: reads the pointer and our window's size; shared cursors need no release.
            unsafe {
                let (mut point, mut rect) = (POINT::default(), RECT::default());
                GetCursorPos(&mut point).unwrap();
                let _ = ScreenToClient(hwnd, &mut point);
                GetClientRect(hwnd, &mut rect).unwrap();
                let id = if point.x < rect.right / 2 {
                    IDC_ARROW
                } else {
                    IDC_IBEAM
                };
                SetCursor(Some(LoadCursorW(None, id).unwrap()));
            }
            LRESULT(1)
        }
        WM_CLOSE => {
            // SAFETY: our window, on its own thread.
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: ends the message loop of this thread.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: the default handling of a message of our window.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn run(x: i32, y: i32, width: i32, height: i32, created: &mpsc::Sender<isize>) {
    // SAFETY: a window of this thread with a static class name and procedure, destroyed on
    // this thread on WM_CLOSE; the message loop ends after that.
    unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            lpszClassName: w!("devcast-input-target"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            ..Default::default()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            w!("devcast-input-target"),
            w!("devcast-input-target"),
            WS_POPUP | WS_VISIBLE | WS_BORDER,
            x,
            y,
            width,
            height,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        created.send(hwnd.0 as isize).unwrap();
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
