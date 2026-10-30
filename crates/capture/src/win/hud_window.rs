//! The app's HUD window as the recording sees it (decision 001): excluded from capture,
//! rounded by DWM, moved in physical desktop pixels. `hwnd` is the window handle of the app.
//! Also bringing a picked window to front.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow,
    SetWindowDisplayAffinity, SetWindowPos, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

use crate::error::CaptureError;

fn handle(hwnd: isize) -> HWND {
    HWND(hwnd as *mut _)
}

/// Hides the window from every screen capture (it stays visible and clickable on screen), or
/// shows it again.
pub fn exclude_from_capture(hwnd: isize, exclude: bool) -> Result<(), CaptureError> {
    let affinity = if exclude {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    // SAFETY: the call only reads the handle; a handle that is not our window makes it fail.
    unsafe { SetWindowDisplayAffinity(handle(hwnd), affinity) }.map_err(|e| {
        CaptureError::platform("скрыть панель из записи")(e.to_string())
    })
}

/// Rounded corners of a borderless window (Windows 11; Windows 10 keeps square corners and
/// returns an error, which callers may ignore only by logging it).
pub fn round_corners(hwnd: isize) -> Result<(), CaptureError> {
    let preference = DWMWCP_ROUND;
    // SAFETY: the pointer and size describe `preference`, which outlives the call.
    unsafe {
        DwmSetWindowAttribute(
            handle(hwnd),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const preference).cast(),
            size_of_val(&preference) as u32,
        )
    }
    .map_err(|e| CaptureError::platform("скруглить углы панели")(e.to_string()))
}

/// Top-left corner and size of the window on the desktop, physical pixels.
pub fn window_rect(hwnd: isize) -> Result<(i32, i32, u32, u32), CaptureError> {
    let mut rect = RECT::default();
    // SAFETY: GetWindowRect writes one RECT into `rect`; a bad handle makes it fail.
    unsafe { GetWindowRect(handle(hwnd), &raw mut rect) }.map_err(|e| {
        CaptureError::platform("узнать место панели")(e.to_string())
    })?;
    let width = (rect.right - rect.left).unsigned_abs();
    let height = (rect.bottom - rect.top).unsigned_abs();
    Ok((rect.left, rect.top, width, height))
}

/// Puts the window's top-left corner at (`x`, `y`), physical desktop pixels.
pub fn move_window(hwnd: isize, x: i32, y: i32) -> Result<(), CaptureError> {
    // SAFETY: no pointers; a bad handle makes the call fail.
    unsafe {
        SetWindowPos(
            handle(hwnd),
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .map_err(|e| CaptureError::platform("передвинуть панель")(e.to_string()))
}

/// Brings the window `id` (a source of the picker) to front, as the user chose it to record.
pub fn bring_to_front(id: u64) -> Result<(), CaptureError> {
    // SAFETY: no pointers; a handle that is not a window makes the call return false.
    let done = unsafe { SetForegroundWindow(HWND(id as *mut _)) };
    if done.as_bool() {
        Ok(())
    } else {
        Err(CaptureError::platform("вывести окно на передний план")(
            "Windows не разрешила".to_owned(),
        ))
    }
}
