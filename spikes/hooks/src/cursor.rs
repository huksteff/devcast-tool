//! Cursor images: system cursor handles → colour bitmap with alpha → PNG, plus hotspot.
//! The cursor type is recognised by comparing HCURSOR with the shared system cursor handles.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, DI_NORMAL, DrawIconEx, GetCursorInfo, GetIconInfo, HCURSOR, ICONINFO, IDC_APPSTARTING, IDC_ARROW,
    IDC_HAND, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT,
    LoadCursorW,
};
use windows::core::PCWSTR;

const SIZE: i32 = 64;

const TYPES: [(&str, PCWSTR); 11] = [
    ("arrow", IDC_ARROW),
    ("ibeam", IDC_IBEAM),
    ("hand", IDC_HAND),
    ("size-we", IDC_SIZEWE),
    ("size-ns", IDC_SIZENS),
    ("size-nwse", IDC_SIZENWSE),
    ("size-nesw", IDC_SIZENESW),
    ("move", IDC_SIZEALL),
    ("no", IDC_NO),
    ("wait", IDC_WAIT),
    ("busy", IDC_APPSTARTING),
];

/// Draws `cursor` into a 64×64 premultiplied BGRA bitmap and returns RGBA bytes + hotspot.
fn render(cursor: HCURSOR) -> Result<(Vec<u8>, (u32, u32)), Box<dyn std::error::Error>> {
    // SAFETY: GDI objects are created here, used for one DrawIconEx and released before return.
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(cursor.into(), &mut info)?;
        let hotspot = (info.xHotspot, info.yHotspot);
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
        let dc = CreateCompatibleDC(None);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let old = SelectObject(dc, HGDIOBJ(dib.0));
        DrawIconEx(dc, 0, 0, cursor.into(), SIZE, SIZE, 0, None, DI_NORMAL)?;
        let pixels = std::slice::from_raw_parts(bits.cast::<u8>(), (SIZE * SIZE * 4) as usize);
        let mut rgba = Vec::with_capacity(pixels.len());
        for p in pixels.chunks_exact(4) {
            rgba.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(dib.0));
        let _ = DeleteDC(dc);
        Ok((rgba, hotspot))
    }
}

pub fn save_cursor_images(out: &str) -> Result<(), Box<dyn std::error::Error>> {
    for (name, id) in TYPES {
        // SAFETY: loads a shared system cursor; shared cursors must not be destroyed.
        let cursor = unsafe { LoadCursorW(None, id)? };
        let (rgba, hotspot) = render(cursor)?;
        let opaque = rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
        image::save_buffer(format!("{out}/cursor-{name}.png"), &rgba, SIZE as u32, SIZE as u32, image::ExtendedColorType::Rgba8)?;
        println!("cursor {name}: hotspot {hotspot:?}, {opaque} visible pixels");
    }
    let mut info = CURSORINFO { cbSize: size_of::<CURSORINFO>() as u32, ..Default::default() };
    // SAFETY: fills `info` with the current global cursor.
    unsafe { GetCursorInfo(&mut info)? };
    let kind = TYPES
        .iter()
        // SAFETY: shared system cursor lookup, compared by handle only.
        .find(|(_, id)| unsafe { LoadCursorW(None, *id) }.is_ok_and(|c| c == info.hCursor))
        .map_or("unknown/custom", |(name, _)| *name);
    println!("current cursor: {kind} at ({}, {})", info.ptScreenPos.x, info.ptScreenPos.y);
    Ok(())
}
