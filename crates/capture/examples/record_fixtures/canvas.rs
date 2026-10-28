//! The fixtures' own window: white, above other windows. It shows the text typed into it and a
//! black square where each click landed, so a frame of the video can be matched to the click
//! in `cursor.bin`. The cursor is the arrow over the left half and the hand over the right.

use std::sync::{Mutex, OnceLock, mpsc};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_QUALITY,
    EndPaint, FillRect, GetStockObject, HBRUSH, HFONT, InvalidateRect, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, ScreenToClient, SelectObject, SetBkMode, TRANSPARENT, TextOutW, WHITE_BRUSH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos,
    GetForegroundWindow, GetMessageW, HTCLIENT, IDC_ARROW, IDC_HAND, LoadCursorW, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, SetCursor, TranslateMessage, WM_CHAR, WM_CLOSE,
    WM_DESTROY, WM_LBUTTONDOWN, WM_PAINT, WM_RBUTTONDOWN, WM_SETCURSOR, WM_SYSCHAR, WNDCLASSW,
    WS_BORDER, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::w;

struct Content {
    text: String,
    clicks: Vec<(i32, i32)>,
}

/// The text font, made once on the window's thread.
static FONT: OnceLock<isize> = OnceLock::new();

static CONTENT: Mutex<Content> = Mutex::new(Content {
    text: String::new(),
    clicks: Vec::new(),
});

/// Shows the window until dropped.
pub struct Canvas {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl Canvas {
    /// A `width`×`height` window at desktop pixel (`x`, `y`).
    pub fn show(x: i32, y: i32, width: i32, height: i32) -> Self {
        let (created, window) = mpsc::channel();
        let thread = std::thread::spawn(move || run(x, y, width, height, &created));
        Self {
            hwnd: window.recv().unwrap(),
            thread: Some(thread),
        }
    }

    pub fn id(&self) -> u64 {
        self.hwnd as u64
    }

    /// Keyboard input goes to this window now.
    pub fn is_foreground(&self) -> bool {
        // SAFETY: reads which window is in the foreground.
        unsafe { GetForegroundWindow() }.0 as isize == self.hwnd
    }

    /// The text typed into the window; lines end with `\r`.
    pub fn text() -> String {
        CONTENT.lock().unwrap().text.clone()
    }

    /// Starts empty: no text, no clicks.
    pub fn clear(&self) {
        let mut content = CONTENT.lock().unwrap();
        content.text.clear();
        content.clicks.clear();
        // SAFETY: our window; repainting it is always allowed.
        let _ = unsafe { InvalidateRect(Some(HWND(self.hwnd as *mut _)), None, true) };
    }
}

impl Drop for Canvas {
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

fn point(lparam: LPARAM) -> (i32, i32) {
    let x = i32::from((lparam.0 & 0xFFFF) as i16);
    let y = i32::from(((lparam.0 >> 16) & 0xFFFF) as i16);
    (x, y)
}

/// Paints the text, line by line, and the clicks.
///
/// # Safety
/// `hwnd` is our window, called on its thread for WM_PAINT.
unsafe fn paint(hwnd: HWND) {
    let content = CONTENT.lock().unwrap();
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: the caller's contract; the font lives as long as the process.
    unsafe {
        let hdc = BeginPaint(hwnd, &mut ps);
        if let Some(font) = FONT.get() {
            SelectObject(hdc, HFONT(*font as *mut _).into());
        }
        SetBkMode(hdc, TRANSPARENT);
        for (row, line) in content.text.split('\r').enumerate() {
            let wide: Vec<u16> = line.encode_utf16().collect();
            let _ = TextOutW(hdc, 40, 40 + row as i32 * 70, &wide);
        }
        let black = HBRUSH(GetStockObject(BLACK_BRUSH).0);
        for &(x, y) in &content.clicks {
            let square = RECT {
                left: x - 6,
                top: y - 6,
                right: x + 6,
                bottom: y + 6,
            };
            FillRect(hdc, &square, black);
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let redraw = || {
        // SAFETY: our window.
        let _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
    };
    match message {
        WM_CHAR => {
            let typed = char::from_u32(wparam.0 as u32).filter(|c| *c == '\r' || !c.is_control());
            if let Some(c) = typed {
                CONTENT.lock().unwrap().text.push(c);
                redraw();
            }
            LRESULT(0)
        }
        // Alt + letter without a menu would beep into the system sound.
        WM_SYSCHAR => LRESULT(0),
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            CONTENT.lock().unwrap().clicks.push(point(lparam));
            redraw();
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: the system asks us to paint our window.
            unsafe { paint(hwnd) };
            LRESULT(0)
        }
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
                    IDC_HAND
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
            lpszClassName: w!("devcast-fixture-canvas"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            ..Default::default()
        };
        RegisterClassW(&class);
        crate::common::synthetic::english_layout();
        let font = CreateFontW(
            56,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            w!("Segoe UI"),
        );
        FONT.get_or_init(|| font.0 as isize);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            w!("devcast-fixture-canvas"),
            w!("devcast-fixture-canvas"),
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
