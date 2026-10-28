//! Sources the user can pick: monitors and windows, as the platform lists them.

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
