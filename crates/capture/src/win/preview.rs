//! Still pictures of a monitor or window for the source picker and the area freeze frame,
//! through GDI. Not WGC: a WGC session, even for one frame, makes Windows 10 draw the yellow
//! capture border around the source (only Windows 11 lets it be turned off).

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, GetDC, GetDIBits, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindow};
use windows::core::Owned;

use super::monitors;
use crate::error::CaptureError;
use crate::preview::{BgraImage, Preview};

/// `PW_RENDERFULLCONTENT`: the window as DWM shows it, also for DirectX and browser windows
/// (missing from the `windows` crate).
const RENDER_FULL_CONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);

/// Monitor `index` (from 1), longer side at most `max_side` pixels. The HUD is excluded from
/// capture and is not in the picture.
pub fn preview_monitor(index: u32, max_side: u32) -> Result<Preview, CaptureError> {
    let monitors = monitors::list()?;
    let bounds = monitors
        .iter()
        .find(|m| m.index == index)
        .ok_or(CaptureError::MonitorNotFound {
            index,
            count: monitors.len(),
        })?
        .bounds;
    let rect = Rect {
        left: bounds.x(),
        top: bounds.y(),
        width: bounds.width() as i32,
        height: bounds.height() as i32,
    };
    Ok(screen(rect)?.preview(max_side))
}

/// Window `id` (not minimized) as it looks on screen, without the invisible resize border,
/// longer side at most `max_side` pixels.
pub fn preview_window(id: u64, max_side: u32) -> Result<Preview, CaptureError> {
    let hwnd = HWND(id as *mut _);
    // SAFETY: IsWindow and IsIconic accept any value and only check it.
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(CaptureError::WindowNotFound(id));
    }
    // SAFETY: as above.
    if unsafe { IsIconic(hwnd) }.as_bool() {
        return Err(CaptureError::WindowMinimized(id));
    }
    Ok(window(hwnd)?.preview(max_side))
}

/// A rectangle of desktop pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl Rect {
    fn from_win(r: RECT) -> Self {
        Self {
            left: r.left,
            top: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        }
    }

    /// `inner` relative to this rectangle, cut to it; `None` if nothing is left.
    fn crop(self, inner: Self) -> Option<Self> {
        let left = inner.left.max(self.left);
        let top = inner.top.max(self.top);
        let right = (inner.left + inner.width).min(self.left + self.width);
        let bottom = (inner.top + inner.height).min(self.top + self.height);
        (right > left && bottom > top).then_some(Self {
            left: left - self.left,
            top: top - self.top,
            width: right - left,
            height: bottom - top,
        })
    }
}

fn failed(action: &'static str) -> CaptureError {
    CaptureError::platform(action)(windows::core::Error::from_thread().to_string())
}

/// The screen DC, released on drop.
struct ScreenDc(HDC);

impl ScreenDc {
    fn get() -> Result<Self, CaptureError> {
        // SAFETY: GetDC(None) returns the DC of the whole screen or null.
        let dc = unsafe { GetDC(None) };
        if dc.is_invalid() {
            return Err(failed("получить контекст экрана"));
        }
        Ok(Self(dc))
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: the DC came from GetDC(None) and is released once.
        unsafe { ReleaseDC(None, self.0) };
    }
}

/// A bitmap of `width` × `height` selected into a memory DC compatible with the screen.
struct Canvas {
    dc: HDC,
    bitmap: Owned<HBITMAP>,
    previous: HGDIOBJ,
    width: i32,
    height: i32,
}

impl Canvas {
    fn new(screen: &ScreenDc, width: i32, height: i32) -> Result<Self, CaptureError> {
        // SAFETY: a valid screen DC; the results are checked and freed by Drop.
        unsafe {
            let dc = CreateCompatibleDC(Some(screen.0));
            if dc.is_invalid() {
                return Err(failed("создать контекст для снимка"));
            }
            let bitmap = Owned::new(CreateCompatibleBitmap(screen.0, width, height));
            if bitmap.is_invalid() {
                let _ = DeleteDC(dc);
                return Err(failed("создать картинку для снимка"));
            }
            let previous = SelectObject(dc, (*bitmap).into());
            Ok(Self {
                dc,
                bitmap,
                previous,
                width,
                height,
            })
        }
    }

    /// The `area` of the canvas as top-down BGRA rows.
    fn read(&self, area: Rect) -> Result<BgraImage, CaptureError> {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.width,
                biHeight: -self.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let row = self.width as usize * 4;
        let mut all = vec![0u8; row * self.height as usize];
        // SAFETY: the bitmap is not selected while read (GetDIBits requires it): it is put back
        // right after. The buffer holds `height` rows of 32-bit pixels as `info` describes.
        let lines = unsafe {
            SelectObject(self.dc, self.previous);
            let lines = GetDIBits(
                self.dc,
                *self.bitmap,
                0,
                self.height as u32,
                Some(all.as_mut_ptr().cast()),
                &raw mut info,
                DIB_RGB_COLORS,
            );
            SelectObject(self.dc, (*self.bitmap).into());
            lines
        };
        if lines != self.height {
            return Err(failed("прочитать снимок"));
        }
        let (x, width) = (area.left as usize * 4, area.width as usize * 4);
        let bgra = all
            .chunks_exact(row)
            .skip(area.top as usize)
            .take(area.height as usize)
            .flat_map(|line| &line[x..x + width])
            .copied()
            .collect();
        Ok(BgraImage {
            width: area.width as u32,
            height: area.height as u32,
            bgra,
        })
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        // SAFETY: the DC is ours; the bitmap is deselected before `bitmap` frees it.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteDC(self.dc);
        }
    }
}

/// Desktop pixels of `rect` as the screen shows them, without the cursor.
fn screen(rect: Rect) -> Result<BgraImage, CaptureError> {
    let screen = ScreenDc::get()?;
    let canvas = Canvas::new(&screen, rect.width, rect.height)?;
    // SAFETY: both DCs are valid and the canvas is `rect` in size.
    unsafe {
        BitBlt(
            canvas.dc,
            0,
            0,
            rect.width,
            rect.height,
            Some(screen.0),
            rect.left,
            rect.top,
            SRCCOPY,
        )
    }
    .map_err(|e| CaptureError::platform("снять экран")(e.to_string()))?;
    canvas.read(Rect {
        left: 0,
        top: 0,
        ..rect
    })
}

/// The window drawn by DWM into a bitmap of its whole rectangle, cut to the visible frame.
fn window(hwnd: HWND) -> Result<BgraImage, CaptureError> {
    let mut outer = RECT::default();
    // SAFETY: a checked window handle and an out struct.
    unsafe { GetWindowRect(hwnd, &raw mut outer) }
        .map_err(|e| CaptureError::platform("узнать размер окна")(e.to_string()))?;
    let mut frame = RECT::default();
    // SAFETY: DWMWA_EXTENDED_FRAME_BOUNDS writes one RECT, whose size is passed.
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut frame).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .map_err(|e| CaptureError::platform("узнать рамку окна")(e.to_string()))?;
    let outer = Rect::from_win(outer);
    let visible = outer.crop(Rect::from_win(frame)).ok_or_else(|| {
        CaptureError::platform("снять окно")("у окна нет видимой части".into())
    })?;
    let screen = ScreenDc::get()?;
    let canvas = Canvas::new(&screen, outer.width, outer.height)?;
    // SAFETY: a checked window handle and our memory DC of the window's size.
    if !unsafe { PrintWindow(hwnd, canvas.dc, RENDER_FULL_CONTENT) }.as_bool() {
        return Err(failed("снять окно"));
    }
    canvas.read(visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, width: i32, height: i32) -> Rect {
        Rect {
            left,
            top,
            width,
            height,
        }
    }

    #[test]
    fn frame_is_cut_relative_to_the_window() {
        // Windows 10: the resize border is invisible, 7 px left, right and bottom.
        let window = rect(-1927, 100, 814, 607);
        let frame = rect(-1920, 100, 800, 600);
        assert_eq!(window.crop(frame), Some(rect(7, 0, 800, 600)));
        // A maximized window reaches past its monitor; the frame is the monitor.
        let window = rect(-8, -8, 1936, 1056);
        assert_eq!(
            window.crop(rect(0, 0, 1920, 1040)),
            Some(rect(8, 8, 1920, 1040))
        );
        assert_eq!(window.crop(rect(5000, 0, 10, 10)), None);
    }
}
