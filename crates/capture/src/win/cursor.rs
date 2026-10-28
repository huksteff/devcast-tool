//! The current cursor: its kind by comparing the handle with the shared system cursors, its
//! picture from the cursor's bitmaps (decision 009), PNG through the Windows image encoder.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, GetCursorInfo, GetIconInfo, HICON, ICONINFO, IDC_APPSTARTING,
    IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW,
    IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, LoadCursorW,
};
use windows::core::PCWSTR;
use windows_capture::encoder::{ImageEncoder, ImageEncoderPixelFormat, ImageFormat};

use crate::error::CaptureError;
use crate::input::{CursorImage, CursorKind};

const SYSTEM: [(PCWSTR, CursorKind); 13] = [
    (IDC_ARROW, CursorKind::Arrow),
    (IDC_IBEAM, CursorKind::Text),
    (IDC_HAND, CursorKind::Pointer),
    (IDC_CROSS, CursorKind::Crosshair),
    (IDC_SIZEWE, CursorKind::ResizeEw),
    (IDC_SIZENS, CursorKind::ResizeNs),
    (IDC_SIZENWSE, CursorKind::ResizeNwse),
    (IDC_SIZENESW, CursorKind::ResizeNesw),
    (IDC_SIZEALL, CursorKind::Move),
    (IDC_NO, CursorKind::NotAllowed),
    (IDC_WAIT, CursorKind::Wait),
    (IDC_APPSTARTING, CursorKind::Progress),
    (IDC_HELP, CursorKind::Help),
];

/// Handles of the shared system cursors (the same in every process).
pub struct SystemCursors(Vec<(isize, CursorKind)>);

impl SystemCursors {
    pub fn load() -> Result<Self, CaptureError> {
        SYSTEM
            .iter()
            .map(|&(id, kind)| {
                // SAFETY: loads a shared system cursor; shared cursors are never destroyed.
                let cursor = unsafe { LoadCursorW(None, id) }.map_err(|e| {
                    CaptureError::platform("загрузить системные курсоры")(
                        e.to_string(),
                    )
                })?;
                Ok((cursor.0 as isize, kind))
            })
            .collect::<Result<_, _>>()
            .map(Self)
    }

    /// The kind of cursor `handle`: a system one, or the application's own.
    pub fn kind(&self, handle: isize) -> CursorKind {
        self.0
            .iter()
            .find(|(h, _)| *h == handle)
            .map_or(CursorKind::Custom, |(_, kind)| *kind)
    }
}

/// The handle of the cursor now and whether it is shown.
pub fn current() -> Result<(isize, bool), CaptureError> {
    let mut info = CURSORINFO {
        cbSize: size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: fills `info`, whose size is set.
    unsafe { GetCursorInfo(&mut info) }.map_err(|e| {
        CaptureError::platform("узнать текущий курсор")(e.to_string())
    })?;
    let visible = info.flags.0 & CURSOR_SHOWING.0 != 0;
    Ok((info.hCursor.0 as isize, visible))
}

/// The bitmaps `GetIconInfo` hands over; the caller must delete them.
struct IconBitmaps(ICONINFO);

impl Drop for IconBitmaps {
    fn drop(&mut self) {
        for bitmap in [self.0.hbmMask, self.0.hbmColor] {
            if !bitmap.is_invalid() {
                // SAFETY: a bitmap GetIconInfo created for us, deleted once.
                let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
            }
        }
    }
}

/// The picture of cursor `handle` at its own size.
pub fn picture(handle: isize) -> Result<CursorImage, CaptureError> {
    let failed = |e: windows::core::Error| {
        CaptureError::platform("прочитать картинку курсора")(e.to_string())
    };
    let mut info = ICONINFO::default();
    // SAFETY: fills `info` for a cursor handle; the bitmaps are deleted by `IconBitmaps`.
    unsafe { GetIconInfo(HICON(handle as *mut _), &mut info) }.map_err(failed)?;
    let icon = IconBitmaps(info);
    let hotspot = (icon.0.xHotspot, icon.0.yHotspot);
    let (width, mask_height) = size(icon.0.hbmMask)?;
    let mask: Vec<bool> = pixels(icon.0.hbmMask, width, mask_height)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[..3] != [0, 0, 0])
        .collect();
    if icon.0.hbmColor.is_invalid() {
        let height = mask_height / 2;
        let (and, xor) = mask.split_at((width * height) as usize);
        return Ok(CursorImage::from_masks(width, height, hotspot, and, xor));
    }
    let (width, height) = size(icon.0.hbmColor)?;
    let color = pixels(icon.0.hbmColor, width, height)?;
    Ok(CursorImage::from_color(
        width, height, hotspot, &color, &mask,
    ))
}

fn size(bitmap: HBITMAP) -> Result<(u32, u32), CaptureError> {
    let mut info = BITMAP::default();
    // SAFETY: writes at most size_of::<BITMAP>() bytes into `info`.
    let written = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            size_of::<BITMAP>() as i32,
            Some((&raw mut info).cast()),
        )
    };
    match written {
        0 => Err(CaptureError::platform("прочитать картинку курсора")(
            "GetObjectW не вернул размер".to_owned(),
        )),
        _ => Ok((info.bmWidth as u32, info.bmHeight as u32)),
    }
}

/// The bitmap as 32-bit BGRA, rows top to bottom (a 1-bit one comes out black and white).
fn pixels(bitmap: HBITMAP, width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0_u8; (width * height * 4) as usize];
    // SAFETY: a memory DC of our own, deleted below; GetDIBits writes `height` rows of
    // `width` 32-bit pixels, which is the size of `bgra`.
    let rows = unsafe {
        let dc = CreateCompatibleDC(None);
        let rows = GetDIBits(
            dc,
            bitmap,
            0,
            height,
            Some(bgra.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        rows
    };
    match rows {
        0 => Err(CaptureError::platform("прочитать картинку курсора")(
            "GetDIBits не прочитал строки".to_owned(),
        )),
        _ => Ok(bgra),
    }
}

/// PNG with straight alpha (the encoder takes premultiplied pixels).
pub fn encode_png(image: &CursorImage) -> Result<Vec<u8>, CaptureError> {
    ImageEncoder::new(ImageFormat::Png, ImageEncoderPixelFormat::Rgba8)
        .and_then(|e| e.encode(&image.premultiplied(), image.width, image.height))
        .map_err(|e| {
            CaptureError::platform("сохранить картинку курсора")(
                e.to_string(),
            )
        })
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
