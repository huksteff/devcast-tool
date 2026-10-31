//! Monitors: the order and numbering of windows-capture (`Monitor::from_index`, from 1).

use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
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
        scale: scale(HMONITOR(monitor.as_raw_hmonitor()))?,
    })
}
/// Windows scale of the monitor, percent.
fn scale(monitor: HMONITOR) -> Result<u32, CaptureError> {
    let (mut x, mut y) = (0u32, 0u32);
    // SAFETY: the handle comes from EnumDisplayMonitors; `x` and `y` are live u32 outputs.
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut x, &raw mut y) }.map_err(
        |e| CaptureError::platform("узнать масштаб монитора")(e.to_string()),
    )?;
    Ok(percent(x))
}

/// Scale in percent from the effective DPI (96 dpi = 100 %), rounded.
fn percent(dpi: u32) -> u32 {
    if dpi == 0 {
        return 100;
    }
    (dpi * 100 + 48) / 96
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_from_dpi() {
        assert_eq!(percent(96), 100);
        assert_eq!(percent(120), 125);
        assert_eq!(percent(144), 150);
        assert_eq!(percent(192), 200);
        assert_eq!(percent(0), 100);
    }

    #[test]
    fn every_monitor_has_a_scale() {
        for monitor in list().unwrap() {
            assert!(
                (100..=500).contains(&monitor.scale),
                "{}: {} %",
                monitor.index,
                monitor.scale
            );
        }
    }
}
