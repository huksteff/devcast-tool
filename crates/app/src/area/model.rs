//! The selected rectangle in physical pixels of its monitor: sides even (H.264 needs even
//! sizes, owner's decision), at least 150 px to start, inside the monitor; presets «16:9»
//! (keeps the ratio) and «1080p» (exactly 1920 × 1080, only moves).

/// Smaller areas cannot start a recording.
pub const MIN_SIDE: i32 = 150;
/// Grab zone of a handle around its point, physical pixels (drawn at 10).
pub const GRAB: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Free,
    Wide,
    FullHd,
}

/// Rectangle `x`, `y`, `w`, `h` in monitor pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn too_small(self) -> bool {
        self.w < MIN_SIDE || self.h < MIN_SIDE
    }

    /// Moved by (`dx`, `dy`) and kept inside a monitor of `size`.
    pub fn moved(self, dx: i32, dy: i32, size: (i32, i32)) -> Self {
        Self {
            x: (self.x + dx).clamp(0, (size.0 - self.w).max(0)),
            y: (self.y + dy).clamp(0, (size.1 - self.h).max(0)),
            ..self
        }
    }
}

fn even(side: i32) -> i32 {
    side - side.rem_euclid(2)
}

/// The rectangle dragged from `anchor` to `pointer` (both in monitor pixels) with `preset`,
/// inside a monitor of `size`. «1080p» is centred on the pointer instead.
pub fn dragged(anchor: (i32, i32), pointer: (i32, i32), preset: Preset, size: (i32, i32)) -> Rect {
    let pointer = (pointer.0.clamp(0, size.0), pointer.1.clamp(0, size.1));
    match preset {
        Preset::FullHd => full_hd(pointer, size),
        Preset::Free | Preset::Wide => {
            let mut w = even((pointer.0 - anchor.0).abs());
            let mut h = even((pointer.1 - anchor.1).abs());
            if preset == Preset::Wide {
                h = even(w * 9 / 16);
                let room = if pointer.1 >= anchor.1 {
                    size.1 - anchor.1
                } else {
                    anchor.1
                };
                if h > room {
                    h = even(room);
                    w = even(h * 16 / 9);
                }
            }
            let x = if pointer.0 >= anchor.0 {
                anchor.0
            } else {
                anchor.0 - w
            };
            let y = if pointer.1 >= anchor.1 {
                anchor.1
            } else {
                anchor.1 - h
            };
            Rect { x, y, w, h }
        }
    }
}

/// 1920 × 1080 centred on `center`, inside the monitor; `None`-like zero size when the monitor
/// is smaller (the preset is then off).
pub fn full_hd(center: (i32, i32), size: (i32, i32)) -> Rect {
    let rect = Rect {
        x: center.0 - 960,
        y: center.1 - 540,
        w: 1920,
        h: 1080,
    };
    rect.moved(0, 0, size)
}

/// Whether the monitor can hold «1080p».
pub fn fits_full_hd(size: (i32, i32)) -> bool {
    size.0 >= 1920 && size.1 >= 1080
}

/// The same rectangle under another preset: «16:9» trims the height (or width), «1080p» keeps
/// the centre.
pub fn with_preset(rect: Rect, preset: Preset, size: (i32, i32)) -> Rect {
    match preset {
        Preset::Free => rect,
        Preset::Wide => dragged(
            (rect.x, rect.y),
            (rect.x + rect.w, rect.y + rect.h),
            preset,
            size,
        ),
        Preset::FullHd => full_hd((rect.x + rect.w / 2, rect.y + rect.h / 2), size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (i32, i32) = (2560, 1440);

    #[test]
    fn sides_are_even_in_any_direction() {
        let r = dragged((501, 301), (100, 50), Preset::Free, SCREEN);
        assert_eq!(
            r,
            Rect {
                x: 101,
                y: 51,
                w: 400,
                h: 250
            }
        );
        let r = dragged((10, 10), (1291, 731), Preset::Free, SCREEN);
        assert_eq!((r.w % 2, r.h % 2), (0, 0));
        assert_eq!((r.w, r.h), (1280, 720));
    }

    #[test]
    fn wide_keeps_16_9_and_the_monitor() {
        let r = dragged((0, 0), (1280, 100), Preset::Wide, SCREEN);
        assert_eq!((r.w, r.h), (1280, 720));
        let r = dragged((0, 1000), (2560, 1440), Preset::Wide, SCREEN);
        assert!(r.y + r.h <= 1440 && r.w * 9 / 16 >= r.h - 2, "{r:?}");
        assert_eq!((r.w % 2, r.h % 2), (0, 0));
    }

    #[test]
    fn full_hd_is_exact_and_inside() {
        let r = full_hd((100, 100), SCREEN);
        assert_eq!(
            r,
            Rect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080
            }
        );
        assert!(fits_full_hd(SCREEN));
        assert!(!fits_full_hd((1600, 900)));
    }

    #[test]
    fn small_and_moved() {
        assert!(
            Rect {
                x: 0,
                y: 0,
                w: 120,
                h: 80
            }
            .too_small()
        );
        let r = Rect {
            x: 2000,
            y: 1000,
            w: 400,
            h: 300,
        };
        assert_eq!(
            r.moved(500, 500, SCREEN),
            Rect {
                x: 2160,
                y: 1140,
                w: 400,
                h: 300
            }
        );
    }
}
