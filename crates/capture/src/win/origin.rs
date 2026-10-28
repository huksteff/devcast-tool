//! Where the recorded picture's top-left corner is on the desktop: input telemetry is written
//! relative to it.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};

use super::monitors;
use crate::error::CaptureError;
use crate::source::Source;

/// Desktop pixel of the picture's top-left corner. A window is measured once, now: if it moves
/// during the recording, positions are off by the distance it moved.
pub fn source_origin(source: &Source) -> Result<(i32, i32), CaptureError> {
    match *source {
        Source::Monitor(index) => {
            let monitors = monitors::list()?;
            let count = monitors.len();
            let monitor = monitors.into_iter().find(|m| m.index == index);
            let bounds = monitor
                .ok_or(CaptureError::MonitorNotFound { index, count })?
                .bounds;
            Ok((bounds.x(), bounds.y()))
        }
        Source::Area(area) => Ok((area.x(), area.y())),
        Source::Window(id) => window_corner(id),
    }
}

/// The corner of the window's visible frame (without the invisible resize border), which is
/// what Windows Graphics Capture shows.
fn window_corner(id: u64) -> Result<(i32, i32), CaptureError> {
    let mut rect = RECT::default();
    // SAFETY: DWMWA_EXTENDED_FRAME_BOUNDS writes one RECT into `rect`, whose size is passed; a
    // handle that is not a window makes the call fail.
    unsafe {
        DwmGetWindowAttribute(
            HWND(id as *mut _),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut rect).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .map_err(|_| CaptureError::WindowNotFound(id))?;
    Ok((rect.left, rect.top))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Area;

    #[test]
    fn monitor_and_area_corners() {
        let monitors = monitors::list().unwrap();
        for monitor in &monitors {
            let b = monitor.bounds;
            assert_eq!(source_origin(&monitor.source()).unwrap(), (b.x(), b.y()));
        }
        let area = Source::Area(Area::new(-1700, 150, 1280, 720).unwrap());
        assert_eq!(source_origin(&area).unwrap(), (-1700, 150));
        let missing = Source::Monitor(monitors.len() as u32 + 1);
        assert!(matches!(
            source_origin(&missing),
            Err(CaptureError::MonitorNotFound { .. })
        ));
        assert!(matches!(
            source_origin(&Source::Window(1)),
            Err(CaptureError::WindowNotFound(1))
        ));
    }
}
