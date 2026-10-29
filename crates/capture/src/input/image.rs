//! Cursor pictures: straight-alpha RGBA with a hotspot, built from the bitmaps of a cursor.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    /// The pixel that points, from the top-left corner.
    pub hotspot: (u32, u32),
    /// `width × height` pixels, rows top to bottom, RGBA with straight (not premultiplied)
    /// alpha.
    pub rgba: Vec<u8>,
}

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

impl CursorImage {
    /// A colour cursor: `bgra` pixels of its colour bitmap and `and` of its mask (true =
    /// transparent). A bitmap with any alpha is taken with its alpha; one without alpha (old
    /// cursors) is opaque where the mask is false.
    pub fn from_color(
        width: u32,
        height: u32,
        hotspot: (u32, u32),
        bgra: &[u8],
        and: &[bool],
    ) -> Self {
        let has_alpha = bgra.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        let rgba = bgra
            .as_chunks::<4>()
            .0
            .iter()
            .zip(and)
            .flat_map(|(p, &clear)| {
                let alpha = match (has_alpha, clear) {
                    (true, _) => p[3],
                    (false, true) => 0,
                    (false, false) => 255,
                };
                [p[2], p[1], p[0], alpha]
            })
            .collect();
        Self {
            width,
            height,
            hotspot,
            rgba,
        }
    }

    /// A monochrome cursor (the I-beam) from its AND and XOR masks:
    ///
    /// | AND | XOR | Pixel |
    /// |---|---|---|
    /// | 0 | 0 | black |
    /// | 0 | 1 | white |
    /// | 1 | 0 | transparent |
    /// | 1 | 1 | «invert the screen»: black with a 1 px white outline |
    ///
    /// The picture is 1 px larger on every side, so that the outline always fits; the hotspot
    /// moves with it.
    pub fn from_masks(
        width: u32,
        height: u32,
        hotspot: (u32, u32),
        and: &[bool],
        xor: &[bool],
    ) -> Self {
        let (w, h) = (width as usize + 2, height as usize + 2);
        let mut rgba = vec![CLEAR; w * h];
        let mut invert = vec![false; w * h];
        for (i, (&a, &x)) in and.iter().zip(xor).enumerate() {
            let at = (i / width as usize + 1) * w + i % width as usize + 1;
            rgba[at] = match (a, x) {
                (false, false) => BLACK,
                (false, true) => WHITE,
                (true, false) => CLEAR,
                (true, true) => BLACK,
            };
            invert[at] = a && x;
        }
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                if invert[y * w + x] {
                    for (dx, dy) in [
                        (0, 1),
                        (2, 1),
                        (1, 0),
                        (1, 2),
                        (0, 0),
                        (2, 0),
                        (0, 2),
                        (2, 2),
                    ] {
                        let around = (y + dy - 1) * w + x + dx - 1;
                        if rgba[around] == CLEAR {
                            rgba[around] = WHITE;
                        }
                    }
                }
            }
        }
        Self {
            width: w as u32,
            height: h as u32,
            hotspot: (hotspot.0 + 1, hotspot.1 + 1),
            rgba: rgba.concat(),
        }
    }

    /// Pixels with alpha multiplied in, as image encoders take them.
    pub fn premultiplied(&self) -> Vec<u8> {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let a = u16::from(p[3]);
                let mul = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
                [mul(p[0]), mul(p[1]), mul(p[2]), p[3]]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(text: &str) -> Vec<bool> {
        text.chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| c == '1')
            .collect()
    }

    fn picture(image: &CursorImage) -> String {
        image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c = match [p[0], p[3]] {
                    [_, 0] => '.',
                    [0, _] => 'B',
                    _ => 'W',
                };
                if (i + 1) % image.width as usize == 0 {
                    format!("{c}\n")
                } else {
                    c.to_string()
                }
            })
            .collect()
    }

    #[test]
    fn monochrome_masks_follow_the_table_and_invert_gets_an_outline() {
        // Row 0: black, white, transparent; row 1: transparent, invert, transparent.
        let and = bits("001 111");
        let xor = bits("010 010");
        let image = CursorImage::from_masks(3, 2, (1, 1), &and, &xor);
        assert_eq!((image.width, image.height, image.hotspot), (5, 4, (2, 2)));
        assert_eq!(picture(&image), ".....\n.BWW.\n.WBW.\n.WWW.\n");
    }

    #[test]
    fn colour_cursor_takes_its_alpha_or_the_mask() {
        let bgra = [10, 20, 30, 128, 1, 2, 3, 0];
        let with_alpha = CursorImage::from_color(2, 1, (0, 0), &bgra, &[false, false]);
        assert_eq!(with_alpha.rgba, [30, 20, 10, 128, 3, 2, 1, 0]);
        let old = [10, 20, 30, 0, 1, 2, 3, 0];
        let masked = CursorImage::from_color(2, 1, (0, 0), &old, &[false, true]);
        assert_eq!(masked.rgba, [30, 20, 10, 255, 3, 2, 1, 0]);
        assert_eq!(with_alpha.premultiplied(), [15, 10, 5, 128, 0, 0, 0, 0]);
    }
}
