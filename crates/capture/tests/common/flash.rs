//! A test window that is white in the first 50 ms of every second of the host clock (QPC) and
//! black otherwise: the reference flash of the sync test, on the same clock as the recording.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use dc_capture::win::now;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, EndPaint, FillRect, GetStockObject, HBRUSH, InvalidateRect,
    PAINTSTRUCT, UpdateWindow, WHITE_BRUSH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG,
    PM_REMOVE, PeekMessageW, RegisterClassW, TranslateMessage, WM_PAINT, WNDCLASSW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::w;

/// Flash length.
pub const FLASH_SECONDS: f64 = 0.05;

static WHITE: AtomicBool = AtomicBool::new(false);

/// Shows the window until dropped.
pub struct Flasher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Flasher {
    /// A `width`×`height` window at desktop pixel (`x`, `y`), above other windows.
    pub fn show(x: i32, y: i32, width: i32, height: i32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&stop);
        let thread = std::thread::spawn(move || run(x, y, width, height, &running));
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Flasher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
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
    if message == WM_PAINT {
        let mut paint = PAINTSTRUCT::default();
        let brush = if WHITE.load(Ordering::Relaxed) {
            WHITE_BRUSH
        } else {
            BLACK_BRUSH
        };
        // SAFETY: painting our own window inside its WM_PAINT; stock brushes need no release.
        unsafe {
            let dc = BeginPaint(hwnd, &mut paint);
            FillRect(dc, &paint.rcPaint, HBRUSH(GetStockObject(brush).0));
            let _ = EndPaint(hwnd, &paint);
        }
        return LRESULT(0);
    }
    // SAFETY: the default handling of a message of our window.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn run(x: i32, y: i32, width: i32, height: i32, stop: &AtomicBool) {
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        lpszClassName: w!("devcast-sync-flash"),
        ..Default::default()
    };
    // SAFETY: a window of this thread with a static class name and procedure; it is destroyed
    // on this thread before it ends.
    let hwnd = unsafe {
        RegisterClassW(&class);
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("devcast-sync-flash"),
            w!("devcast-sync-flash"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            None,
            None,
        )
    }
    .unwrap();
    let mut message = MSG::default();
    while !stop.load(Ordering::Relaxed) {
        let into = now().unwrap().micros().rem_euclid(1_000_000) as f64 / 1e6;
        let white = into < FLASH_SECONDS;
        if WHITE.swap(white, Ordering::Relaxed) != white {
            // SAFETY: our window, alive until the end of this function.
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
                let _ = UpdateWindow(hwnd);
            }
        }
        // SAFETY: the message queue of this thread.
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        // High-resolution sleep: the flash starts within a millisecond of its second.
        std::thread::sleep(Duration::from_millis(1));
    }
    // SAFETY: our window, created above on this thread.
    let _ = unsafe { DestroyWindow(hwnd) };
}
