//! Monitors: the order and numbering of windows-capture (`Monitor::from_index`, from 1).

use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows_capture::monitor::Monitor;

use crate::error::CaptureError;
use crate::list::MonitorInfo;
use crate::source::Area;

pub fn list() -> Result<Vec<MonitorInfo>, CaptureError> {
    let monitors = Monitor::enumerate().map_err(|e| {
        CaptureError::platform("получить список мониторов")(e.to_string())
    })?;
    monitors
        .iter()
        .zip(1..)
        .map(|(monitor, index)| info(monitor, index))
        .collect()
}

/// The monitor with number `index` (from 1).
pub fn by_index(index: u32) -> Result<Monitor, CaptureError> {
    let monitors = Monitor::enumerate().map_err(|e| {
        CaptureError::platform("получить список мониторов")(e.to_string())
    })?;
    let found = (index as usize)
        .checked_sub(1)
        .and_then(|i| monitors.get(i));
    found.copied().ok_or(CaptureError::MonitorNotFound {
        index,
        count: monitors.len(),
    })
}

fn info(monitor: &Monitor, index: u32) -> Result<MonitorInfo, CaptureError> {
    let mut raw = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: the handle comes from EnumDisplayMonitors and `raw` is a MONITORINFO with cbSize set.
    let ok = unsafe { GetMonitorInfoW(HMONITOR(monitor.as_raw_hmonitor()), &raw mut raw) };
    if !ok.as_bool() {
        return Err(CaptureError::platform("узнать положение монитора")(
            windows::core::Error::from_thread().to_string(),
        ));
    }
    let r = raw.rcMonitor;
    let size = |from: i32, to: i32| u32::try_from(to - from).unwrap_or(0);
    let bounds =
        Area::new(r.left, r.top, size(r.left, r.right), size(r.top, r.bottom)).map_err(|e| {
            CaptureError::platform("узнать размер монитора")(e.to_string())
        })?;
    Ok(MonitorInfo {
        index,
        name: monitor.name().ok().filter(|n| !n.trim().is_empty()),
        bounds,
        primary: raw.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}
