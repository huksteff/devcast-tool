//! Where the HUD stands: its place is remembered for each monitor (by the monitor's desktop
//! rectangle) as an offset from the monitor's corner, in physical pixels. A new monitor gets
//! the bar at the bottom centre.

use std::collections::BTreeMap;

use dc_capture::{Area, MonitorInfo};
use serde::{Deserialize, Serialize};

/// Gap between the bar and the bottom of a monitor it was never moved on, physical pixels.
const BOTTOM_GAP: i32 = 96;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HudPlacement {
    /// Key of the monitor the bar was on last.
    pub last: Option<String>,
    /// Offset of the bar's corner from each monitor's corner.
    pub offsets: BTreeMap<String, [i32; 2]>,
}

/// `2560x1440@0,0`: a monitor is known by where it is and how big it is.
fn key(bounds: &Area) -> String {
    format!(
        "{}x{}@{},{}",
        bounds.width(),
        bounds.height(),
        bounds.x(),
        bounds.y()
    )
}

impl HudPlacement {
    /// Corner of a bar of `size` on the last monitor (or the primary one when it is gone).
    pub fn place(&self, monitors: &[MonitorInfo], size: (u32, u32)) -> Option<(i32, i32)> {
        let b = self.monitor(monitors)?.bounds;
        let [dx, dy] = self.offsets.get(&key(&b)).copied().unwrap_or([
            (b.width() as i32 - size.0 as i32) / 2,
            b.height() as i32 - size.1 as i32 - BOTTOM_GAP,
        ]);
        Some((b.x() + dx, b.y() + dy))
    }

    /// The monitor of the bar: the last one it stood on, or the primary one when it is gone.
    pub fn monitor<'a>(&self, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo> {
        let last = self
            .last
            .as_ref()
            .and_then(|last| monitors.iter().find(|m| &key(&m.bounds) == last));
        last.or_else(|| monitors.iter().find(|m| m.primary))
    }

    /// Remembers the bar at `rect` (corner and size) on the monitor under its centre. Returns
    /// false when no monitor has it.
    pub fn remember(&mut self, monitors: &[MonitorInfo], rect: (i32, i32, u32, u32)) -> bool {
        let center = (rect.0 + rect.2 as i32 / 2, rect.1 + rect.3 as i32 / 2);
        let Some(monitor) = monitors.iter().find(|m| contains(&m.bounds, center)) else {
            return false;
        };
        let b = monitor.bounds;
        let name = key(&b);
        self.offsets
            .insert(name.clone(), [rect.0 - b.x(), rect.1 - b.y()]);
        self.last = Some(name);
        true
    }
}

fn contains(b: &Area, (x, y): (i32, i32)) -> bool {
    x >= b.x() && y >= b.y() && x < b.x() + b.width() as i32 && y < b.y() + b.height() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo {
                index: 1,
                name: None,
                bounds: Area::new(0, 0, 2560, 1440).unwrap(),
                primary: true,
            },
            MonitorInfo {
                index: 2,
                name: None,
                bounds: Area::new(-1920, 200, 1920, 1080).unwrap(),
                primary: false,
            },
        ]
    }

    #[test]
    fn new_bar_is_at_the_bottom_centre_of_the_primary() {
        let placement = HudPlacement::default();
        assert_eq!(placement.place(&monitors(), (404, 48)), Some((1078, 1296)));
    }

    #[test]
    fn remembers_each_monitor() {
        let mut placement = HudPlacement::default();
        assert!(placement.remember(&monitors(), (-1800, 300, 404, 48)));
        assert_eq!(placement.place(&monitors(), (404, 48)), Some((-1800, 300)));
        assert_eq!(placement.monitor(&monitors()).map(|m| m.index), Some(2));
        assert!(placement.remember(&monitors(), (100, 50, 404, 48)));
        assert_eq!(placement.place(&monitors(), (404, 48)), Some((100, 50)));
        assert_eq!(placement.offsets["1920x1080@-1920,200"], [120, 100]);
        assert!(!placement.remember(&monitors(), (5000, 5000, 404, 48)));
    }

    #[test]
    fn a_gone_monitor_falls_back_to_the_primary() {
        let mut placement = HudPlacement::default();
        placement.remember(&monitors(), (-1800, 300, 404, 48));
        let only_primary = &monitors()[..1];
        assert_eq!(placement.place(only_primary, (404, 48)), Some((1078, 1296)));
    }
}
