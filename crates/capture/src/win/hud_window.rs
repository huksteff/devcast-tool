//! The app's HUD window as the recording sees it (decision 001): excluded from capture, moved
//! in physical desktop pixels. `hwnd` is the window handle of the app. Also bringing a picked
//! window to front, and the work area of the monitor the HUD stands on (where its popovers fit).

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
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

/// The work area (the monitor without the taskbar) of the monitor that holds most of `rect`
/// (or the nearest one); both as [`window_rect`] gives them: corner and size, physical desktop
/// pixels.
pub fn work_area(
    (x, y, width, height): (i32, i32, u32, u32),
) -> Result<(i32, i32, u32, u32), CaptureError> {
    let rect = RECT {
        left: x,
        top: y,
        right: x.saturating_add_unsigned(width),
        bottom: y.saturating_add_unsigned(height),
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `rect` is a live RECT; with MONITOR_DEFAULTTONEAREST the handle is always a monitor,
    // and `info` is a MONITORINFO with cbSize set.
    let ok = unsafe {
        GetMonitorInfoW(
            MonitorFromRect(&raw const rect, MONITOR_DEFAULTTONEAREST),
            &raw mut info,
        )
    };
    if !ok.as_bool() {
        return Err(CaptureError::platform("узнать рабочую область монитора")(
            windows::core::Error::from_thread().to_string(),
        ));
    }
    let w = info.rcWork;
    let size = |from: i32, to: i32| u32::try_from(to - from).unwrap_or(0);
    Ok((w.left, w.top, size(w.left, w.right), size(w.top, w.bottom)))
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
