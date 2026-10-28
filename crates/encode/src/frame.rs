//! A picture for the encoder: NV12 (BT.709 limited range), planes in FFmpeg's own buffer, so
//! filling it is the only copy on the CPU.

use ffmpeg_next as ffmpeg;
use ffmpeg_next::ffi;

use crate::error::EncodeError;

pub struct Nv12Frame {
    pub(crate) frame: ffmpeg::frame::Video,
}

impl Nv12Frame {
    /// A black frame. NV12 halves the color planes, so both sides must be even.
    pub fn new(width: u32, height: u32) -> Result<Self, EncodeError> {
        check_size(width, height)?;
        crate::init();
        let mut frame = Self {
            frame: ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, width, height),
        };
        frame.fill(16, 128, 128);
        Ok(frame)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.frame.width(), self.frame.height())
    }

    /// Copies both planes from memory where each row starts `pitch` bytes after the previous
    /// one: `y` has `height` rows of `width` bytes, `uv` has `height / 2` rows of interleaved
    /// U, V bytes.
    pub fn copy_from(&mut self, y: &[u8], uv: &[u8], pitch: usize) {
        let (width, height) = self.size();
        let (width, height) = (width as usize, height as usize);
        copy_plane(&mut self.frame, 0, y, pitch, width, height);
        copy_plane(&mut self.frame, 1, uv, pitch, width, height / 2);
    }

    /// Fills the frame with one color.
    pub fn fill(&mut self, y: u8, u: u8, v: u8) {
        self.frame.data_mut(0).fill(y);
        for pair in self.frame.data_mut(1).as_chunks_mut::<2>().0 {
            *pair = [u, v];
        }
    }

    /// Rows of the luma plane, `width` bytes each, for building test pictures.
    pub fn luma_rows_mut(&mut self) -> impl Iterator<Item = &mut [u8]> {
        let width = self.frame.width() as usize;
        let stride = self.frame.stride(0);
        self.frame
            .data_mut(0)
            .chunks_exact_mut(stride)
            .map(move |row| &mut row[..width])
    }

    /// Marks the frame as an IDR key frame or clears the mark; the frame is reused, so every
    /// frame sets it.
    pub(crate) fn set_key(&mut self, key: bool) {
        let kind = if key {
            ffmpeg::picture::Type::I
        } else {
            ffmpeg::picture::Type::None
        };
        self.frame.set_kind(kind);
        // SAFETY: `flags` is a plain int field of the owned AVFrame.
        unsafe {
            let flags = &mut (*self.frame.as_mut_ptr()).flags;
            if key {
                *flags |= ffi::AV_FRAME_FLAG_KEY;
            } else {
                *flags &= !ffi::AV_FRAME_FLAG_KEY;
            }
        }
    }
}

pub(crate) fn check_size(width: u32, height: u32) -> Result<(), EncodeError> {
    if width < 2 || height < 2 || !width.is_multiple_of(2) || !height.is_multiple_of(2) {
        return Err(EncodeError::FrameSize { width, height });
    }
    Ok(())
}

fn copy_plane(
    frame: &mut ffmpeg::frame::Video,
    plane: usize,
    src: &[u8],
    pitch: usize,
    width: usize,
    rows: usize,
) {
    let stride = frame.stride(plane);
    let dst = frame.data_mut(plane);
    for (to, from) in dst
        .chunks_exact_mut(stride)
        .zip(src.chunks(pitch))
        .take(rows)
    {
        to[..width].copy_from_slice(&from[..width]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_sizes_are_refused() {
        for (w, h) in [(0, 2), (2, 0), (641, 360), (640, 361), (1, 1)] {
            assert!(matches!(
                Nv12Frame::new(w, h),
                Err(EncodeError::FrameSize { .. })
            ));
        }
    }

    #[test]
    fn copies_rows_with_pitch() {
        let mut frame = Nv12Frame::new(4, 2).unwrap();
        let y: Vec<u8> = (0..16).collect();
        let uv: Vec<u8> = (100..104).collect();
        frame.copy_from(&y, &uv, 8);
        let rows: Vec<Vec<u8>> = frame.luma_rows_mut().map(|r| r.to_vec()).collect();
        assert_eq!(rows, [vec![0, 1, 2, 3], vec![8, 9, 10, 11]]);
        assert_eq!(&frame.frame.data(1)[..4], &[100, 101, 102, 103]);
    }
}
