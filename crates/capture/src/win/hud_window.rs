//! The app's HUD window as the recording sees it (decision 001): excluded from capture,
//! cut to a rounded rectangle, moved in physical desktop pixels. `hwnd` is the window handle of the app.
//! Also bringing a picked window to front.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
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

/// Cuts the window to a `width` × `height` rectangle with corners of `radius`, physical
/// pixels: the corners outside show what is under the window. The same on Windows 10 and 11
/// (DWM rounds only on 11, and a see-through eframe window shows white there, decision 001).
pub fn round_window(hwnd: isize, width: u32, height: u32, radius: u32) -> Result<(), CaptureError> {
    let failed = CaptureError::platform("скруглить углы панели");
    let (w, h, d) = (width as i32 + 1, height as i32 + 1, 2 * radius as i32);
    // SAFETY: no pointers; the region is ours until SetWindowRgn takes it.
    let region = unsafe { CreateRoundRectRgn(0, 0, w, h, d, d) };
    if region.is_invalid() {
        return Err(failed("CreateRoundRectRgn".to_owned()));
    }
    // SAFETY: on success the system owns the region; a bad handle makes the call fail.
    if unsafe { SetWindowRgn(handle(hwnd), Some(region), true) } == 0 {
        // SAFETY: the call failed, so the region is still ours to delete.
        let _ = unsafe { DeleteObject(region.into()) };
        return Err(failed("SetWindowRgn".to_owned()));
    }
    Ok(())
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
