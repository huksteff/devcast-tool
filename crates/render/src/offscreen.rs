//! Frames without a window (`devcast frame` and `sheet`, reference frames): a scene drawn into a
//! texture of its own size and read back, and frames laid out on one sheet.

use std::num::NonZeroU32;

use dc_core::PixelSize;

use crate::compositor::Compositor;
use crate::error::RenderError;
use crate::gpu::Gpu;
use crate::readback::Image;
use crate::scene::Scene;

/// Space between the frames of a sheet, px.
pub const SHEET_GAP: u32 = 8;
/// Colour of that space: mid grey, so black fields of the frames stay visible.
const GAP_RGBA: [u8; 4] = [128, 128, 128, 255];

impl Compositor {
    /// `scene` drawn at its own size and read back.
    pub fn render(&mut self, gpu: &Gpu, scene: &Scene) -> Result<Image, RenderError> {
        let target = gpu.target(scene.size);
        self.draw(gpu, scene, &target)?;
        Ok(gpu.read(&target)?)
    }
}

impl Image {
    /// Frames of one size laid out in rows of `columns`, left to right and top to bottom, with
    /// [`SHEET_GAP`] between them and around.
    pub fn sheet(frames: &[Image], columns: NonZeroU32) -> Result<Image, RenderError> {
        let first = frames.first().ok_or(RenderError::EmptySheet)?;
        let cell = first.size;
        if let Some(other) = frames.iter().find(|f| f.size != cell) {
            return Err(RenderError::SheetSizes {
                first: cell,
                other: other.size,
            });
        }
        let count = frames.len() as u32;
        let columns = columns.get().min(count);
        let rows = count.div_ceil(columns);
        let size = PixelSize {
            width: columns * (cell.width + SHEET_GAP) + SHEET_GAP,
            height: rows * (cell.height + SHEET_GAP) + SHEET_GAP,
        };
        let mut rgba = GAP_RGBA.repeat(size.width as usize * size.height as usize);
        let row = cell.width as usize * 4;
        for (i, frame) in (0..).zip(frames) {
            let left = SHEET_GAP + i % columns * (cell.width + SHEET_GAP);
            let top = SHEET_GAP + i / columns * (cell.height + SHEET_GAP);
            for (y, line) in (top..).zip(frame.rgba.chunks_exact(row)) {
                let at = (y * size.width + left) as usize * 4;
                rgba[at..at + row].copy_from_slice(line);
            }
        }
        Ok(Image { size, rgba })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(width: u32, height: u32, value: u8) -> Image {
        Image {
            size: PixelSize { width, height },
            rgba: vec![value; (width * height * 4) as usize],
        }
    }

    fn at(image: &Image, x: u32, y: u32) -> u8 {
        image.rgba[((y * image.size.width + x) * 4) as usize]
    }

    #[test]
    fn sheet_lays_frames_in_rows_with_gaps() {
        let frames: Vec<_> = (1..=5).map(|v| plain(4, 2, v * 10)).collect();
        let columns = NonZeroU32::new(3).unwrap_or(NonZeroU32::MIN);
        let sheet = Image::sheet(&frames, columns).unwrap();
        let gap = SHEET_GAP;
        assert_eq!(
            sheet.size,
            PixelSize {
                width: 3 * (4 + gap) + gap,
                height: 2 * (2 + gap) + gap
            }
        );
        assert_eq!(at(&sheet, 0, 0), 128);
        assert_eq!(at(&sheet, gap, gap), 10);
        assert_eq!(at(&sheet, gap + 3, gap + 1), 10);
        assert_eq!(at(&sheet, gap + 4, gap), 128);
        assert_eq!(at(&sheet, 2 * gap + 4, gap), 20);
        assert_eq!(at(&sheet, 2 * gap + 4, 2 * gap + 2), 50);
        assert_eq!(at(&sheet, 3 * gap + 8, 2 * gap + 2), 128, "no sixth frame");
    }

    #[test]
    fn one_row_is_never_wider_than_the_frames() {
        let frames = [plain(4, 2, 10), plain(4, 2, 20)];
        let columns = NonZeroU32::new(6).unwrap_or(NonZeroU32::MIN);
        let sheet = Image::sheet(&frames, columns).unwrap();
        assert_eq!(sheet.size.width, 2 * (4 + SHEET_GAP) + SHEET_GAP);
    }

    #[test]
    fn sheet_needs_frames_of_one_size() {
        let columns = NonZeroU32::MIN;
        assert!(matches!(
            Image::sheet(&[], columns),
            Err(RenderError::EmptySheet)
        ));
        assert!(matches!(
            Image::sheet(&[plain(4, 2, 0), plain(2, 4, 0)], columns),
            Err(RenderError::SheetSizes { .. })
        ));
    }
}
