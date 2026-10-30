//! Frames of a video as 1280×720 gray pixels, compared region by region (tests of task 5.39).

use std::path::Path;

use crate::test_util::decoded;

const WIDTH: usize = 1280;
const HEIGHT: usize = 720;
/// Brightness difference (0..255) that counts a pixel as changed: more than coding noise.
const CHANGED: i16 = 60;

pub(crate) struct Frame(Vec<u8>);

/// A part of the frame, fractions 0..1 like `--rect`.
pub(crate) struct Region {
    x: (usize, usize),
    y: (usize, usize),
}

impl Region {
    pub(crate) fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        let px = |f: f64, size: usize| ((f * size as f64).round() as usize).min(size);
        Self {
            x: (px(x, WIDTH), px(x + w, WIDTH)),
            y: (px(y, HEIGHT), px(y + h, HEIGHT)),
        }
    }
}

impl Frame {
    /// What `file` shows at `secs` (to 1/60 s). A recording keeps a frame only when the screen
    /// changed, so the frames are first made constant-rate.
    pub(crate) fn at(file: &Path, secs: f64) -> Self {
        let file = file.to_str().unwrap();
        let filter = format!("fps=60,trim=start={secs:.4},scale={WIDTH}:{HEIGHT},format=gray");
        let pixels = decoded(&[
            "-i",
            file,
            "-vf",
            &filter,
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-",
        ]);
        assert_eq!(pixels.len(), WIDTH * HEIGHT, "{file} at {secs}");
        Self(pixels)
    }

    fn pixel(&self, x: usize, y: usize) -> u8 {
        self.0[y * WIDTH + x]
    }

    /// Pixels of `region` that differ from `other` by more than coding noise.
    pub(crate) fn changed(&self, other: &Frame, region: &Region) -> usize {
        (region.y.0..region.y.1)
            .flat_map(|y| (region.x.0..region.x.1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let d = i16::from(self.pixel(x, y)) - i16::from(other.pixel(x, y));
                d.abs() > CHANGED
            })
            .count()
    }

    /// Share of the outermost pixels that are darker than the white window: its 1 px gray
    /// frame is on the picture edge only when nothing zooms in.
    pub(crate) fn edge_dark(&self) -> f64 {
        let top = (0..WIDTH).flat_map(|x| [(x, 0), (x, HEIGHT - 1)]);
        let sides = (0..HEIGHT).flat_map(|y| [(0, y), (WIDTH - 1, y)]);
        let edge: Vec<u8> = top.chain(sides).map(|(x, y)| self.pixel(x, y)).collect();
        edge.iter().filter(|&&v| v < 200).count() as f64 / edge.len() as f64
    }
}
