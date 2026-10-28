//! Sources the user can pick: monitors and windows, as the platform lists them.

use crate::error::CaptureError;
use crate::source::{Area, Source};

/// Everything that can be recorded right now. Windows are in z-order, topmost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceList {
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
}

impl SourceList {
    /// The monitor with the taskbar and the Start menu; recording uses it by default.
    pub fn primary(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.primary)
    }
}

/// The monitor that holds all of `area`, and `area` in that monitor's pixels.
pub fn area_on_monitor(
    monitors: &[MonitorInfo],
    area: Area,
) -> Result<(&MonitorInfo, Area), CaptureError> {
    let inside = |m: &&MonitorInfo| {
        let b = m.bounds;
        let end = |start: i32, len: u32| i64::from(start) + i64::from(len);
        area.x() >= b.x()
            && area.y() >= b.y()
            && end(area.x(), area.width()) <= end(b.x(), b.width())
            && end(area.y(), area.height()) <= end(b.y(), b.height())
    };
    let monitor = monitors
        .iter()
        .find(inside)
        .ok_or(CaptureError::AreaOffMonitors(Source::Area(area)))?;
    let local = Area::new(
        area.x() - monitor.bounds.x(),
        area.y() - monitor.bounds.y(),
        area.width(),
        area.height(),
    )?;
    Ok((monitor, local))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    /// Number in [`Source::Monitor`], from 1.
    pub index: u32,
    /// Model name from the monitor itself (EDID), if it reports one.
    pub name: Option<String>,
    /// Position and size on the virtual desktop, in physical pixels.
    pub bounds: Area,
    pub primary: bool,
}

impl MonitorInfo {
    pub const fn source(&self) -> Source {
        Source::Monitor(self.index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// Number in [`Source::Window`].
    pub id: u64,
    pub title: String,
    /// Program file name without extension (`chrome`, `Code`), if the system tells it.
    pub app: Option<String>,
    /// A minimized window has no picture until it is restored.
    pub minimized: bool,
}

impl WindowInfo {
    pub const fn source(&self) -> Source {
        Source::Window(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(index: u32, x: i32, y: i32) -> MonitorInfo {
        MonitorInfo {
            index,
            name: None,
            bounds: Area::new(x, y, 1920, 1080).unwrap(),
            primary: index == 1,
        }
    }

    #[test]
    fn area_is_moved_into_its_monitor() {
        let monitors = [monitor(1, 0, 0), monitor(2, -1920, 11)];
        let area = Area::new(-1500, 261, 640, 360).unwrap();
        let (found, local) = area_on_monitor(&monitors, area).unwrap();
        assert_eq!(found.index, 2);
        assert_eq!(local, Area::new(420, 250, 640, 360).unwrap());
        let whole = Area::new(0, 0, 1920, 1080).unwrap();
        assert_eq!(area_on_monitor(&monitors, whole).unwrap().1, whole);
    }

    #[test]
    fn area_must_fit_one_monitor() {
        let monitors = [monitor(1, 0, 0), monitor(2, -1920, 11)];
        for (x, y, w, h) in [
            (-100, 100, 200, 100),
            (1800, 0, 121, 10),
            (0, 1000, 10, 81),
            (-1920, 0, 10, 10),
            (5000, 0, 10, 10),
        ] {
            let area = Area::new(x, y, w, h).unwrap();
            let error = area_on_monitor(&monitors, area).unwrap_err();
            assert!(matches!(error, CaptureError::AreaOffMonitors(_)), "{error}");
        }
    }
}
