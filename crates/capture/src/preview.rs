//! Small RGBA pictures of sources for the source picker.

/// A captured frame in memory: 8-bit BGRA rows without padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgraImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// A downscaled picture: 8-bit RGBA rows without padding (ready for an egui texture).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl BgraImage {
    /// Shrinks the image so its longer side is at most `max_side` (keeping the aspect ratio),
    /// averaging every source pixel into the preview pixel it falls into.
    pub fn preview(&self, max_side: u32) -> Preview {
        let longer = self.width.max(self.height);
        let scale = |side: u32| {
            if longer <= max_side {
                side
            } else {
                (u64::from(side) * u64::from(max_side) / u64::from(longer)).max(1) as u32
            }
        };
        let (width, height) = (scale(self.width), scale(self.height));
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for py in 0..height {
            let (y0, y1) = span(py, height, self.height);
            for px in 0..width {
                let (x0, x1) = span(px, width, self.width);
                let mut sum = [0u64; 4];
                for y in y0..y1 {
                    let row = y as usize * self.width as usize;
                    for x in x0..x1 {
                        let at = (row + x as usize) * 4;
                        for (total, value) in sum.iter_mut().zip(&self.bgra[at..at + 4]) {
                            *total += u64::from(*value);
                        }
                    }
                }
                let count = u64::from((x1 - x0) * (y1 - y0));
                let [b, g, r, a] = sum.map(|total| (total / count) as u8);
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
        Preview {
            width,
            height,
            rgba,
        }
    }
}

/// Source pixels `start..end` covered by preview pixel `i` of `small`, out of `big` (never empty).
fn span(i: u32, small: u32, big: u32) -> (u32, u32) {
    let at = |i: u32| (u64::from(i) * u64::from(big) / u64::from(small)) as u32;
    let start = at(i);
    (start, at(i + 1).max(start + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> BgraImage {
        let bgra = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| pixel(x, y))
            .collect();
        BgraImage {
            width,
            height,
            bgra,
        }
    }

    #[test]
    fn keeps_aspect_and_limits_longer_side() {
        let p = image(1920, 1080, |_, _| [0, 0, 0, 255]).preview(320);
        assert_eq!((p.width, p.height), (320, 180));
        let p = image(300, 1000, |_, _| [0, 0, 0, 255]).preview(100);
        assert_eq!((p.width, p.height), (30, 100));
        let p = image(64, 48, |_, _| [0, 0, 0, 255]).preview(320);
        assert_eq!((p.width, p.height), (64, 48));
        let p = image(5000, 2, |_, _| [0, 0, 0, 255]).preview(100);
        assert_eq!((p.width, p.height), (100, 1));
        assert_eq!(p.rgba.len(), 100 * 4);
    }

    #[test]
    fn swaps_channels_and_averages() {
        let p = image(2, 2, |_, _| [10, 20, 30, 255]).preview(2);
        assert_eq!(&p.rgba[..4], &[30, 20, 10, 255]);
        // Left half black, right half white: one preview pixel per half.
        let p = image(4, 2, |x, _| if x < 2 { [0, 0, 0, 255] } else { [255; 4] }).preview(2);
        assert_eq!((p.width, p.height), (2, 1));
        assert_eq!(p.rgba, [0, 0, 0, 255, 255, 255, 255, 255]);
        // Checkerboard averages to grey.
        let p = image(4, 4, |x, y| {
            if (x + y) % 2 == 0 {
                [0, 0, 0, 255]
            } else {
                [254, 254, 254, 255]
            }
        })
        .preview(1);
        assert_eq!(p.rgba, [127, 127, 127, 255]);
    }
}
