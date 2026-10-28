//! A captured frame as the sink sees it: the whole capture or the area cut out on the GPU.

use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device,
    ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows_capture::frame::Frame;

use super::{clock, d3d};
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::preview::BgraImage;
use crate::source::{Area, Source};

/// One captured frame, valid only inside [`super::FrameSink::frame`].
pub struct VideoFrame<'a> {
    texture: &'a ID3D11Texture2D,
    device: &'a ID3D11Device,
    context: &'a ID3D11DeviceContext,
    size: (u32, u32),
    time: HostTime,
}

impl VideoFrame<'_> {
    /// The frame on the GPU, BGRA8; reused for the next frame, so copy or encode it here.
    pub fn texture(&self) -> &ID3D11Texture2D {
        self.texture
    }

    /// Width and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// When the frame was shown on screen, on the QPC timeline.
    pub fn time(&self) -> HostTime {
        self.time
    }

    /// Copies the frame to memory (slow: for previews and checks, not for every frame).
    pub fn to_bgra(&self) -> Result<BgraImage, CaptureError> {
        d3d::read_bgra(self.device, self.context, self.texture)
    }
}

/// Cuts an area out of every frame with `CopySubresourceRegion` into a texture of its own.
pub struct Crop {
    /// The area in the captured monitor's pixels.
    local: Area,
    /// The same area on the desktop, for error messages.
    desktop: Area,
    texture: ID3D11Texture2D,
}

/// Where to cut, before the capture has a device.
pub struct CropArea {
    pub local: Area,
    pub desktop: Area,
}

impl Crop {
    pub fn new(area: CropArea, device: &ID3D11Device) -> Result<Self, CaptureError> {
        Ok(Self {
            texture: create_texture(device, area.local)?,
            local: area.local,
            desktop: area.desktop,
        })
    }
}

/// Wraps the frame for the sink, cutting out `crop` if there is one.
pub fn view<'a>(
    frame: &'a Frame<'a>,
    crop: Option<&'a Crop>,
) -> Result<VideoFrame<'a>, CaptureError> {
    let time = frame.timestamp().map_err(|e| {
        CaptureError::platform("получить время кадра")(e.to_string())
    })?;
    let (device, context) = (frame.device(), frame.device_context());
    let Some(crop) = crop else {
        return Ok(VideoFrame {
            texture: frame.as_raw_texture(),
            device,
            context,
            size: (frame.width(), frame.height()),
            time: clock::from_system_relative(time),
        });
    };
    let a = crop.local;
    let fits = |start: i32, len: u32, limit: u32| {
        let start = u32::try_from(start).ok()?;
        let end = start.checked_add(len).filter(|end| *end <= limit)?;
        Some((start, end))
    };
    let horizontal = fits(a.x(), a.width(), frame.width());
    let vertical = fits(a.y(), a.height(), frame.height());
    let (Some((left, right)), Some((top, bottom))) = (horizontal, vertical) else {
        return Err(CaptureError::AreaOutsideFrame {
            area: Source::Area(crop.desktop),
            width: frame.width(),
            height: frame.height(),
        });
    };
    let texture = &crop.texture;
    let region = D3D11_BOX {
        left,
        top,
        front: 0,
        right,
        bottom,
        back: 1,
    };
    // SAFETY: same format; `region` lies inside the source (checked by `fits`) and has the size
    // of the destination texture.
    unsafe {
        context.CopySubresourceRegion(
            texture,
            0,
            0,
            0,
            0,
            frame.as_raw_texture(),
            0,
            Some(&raw const region),
        );
    }
    Ok(VideoFrame {
        texture,
        device,
        context,
        size: (a.width(), a.height()),
        time: clock::from_system_relative(time),
    })
}

/// A BGRA8 texture of the area's size, the format of [`ColorFormat::Bgra8`] frames.
///
/// [`ColorFormat::Bgra8`]: windows_capture::settings::ColorFormat::Bgra8
fn create_texture(device: &ID3D11Device, area: Area) -> Result<ID3D11Texture2D, CaptureError> {
    let failed = |message: String| {
        CaptureError::platform("создать текстуру области")(message)
    };
    let desc = D3D11_TEXTURE2D_DESC {
        Width: area.width(),
        Height: area.height(),
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    // SAFETY: a valid description and an out slot for the new texture.
    unsafe { device.CreateTexture2D(&raw const desc, None, Some(&raw mut texture)) }
        .map_err(|e| failed(e.to_string()))?;
    texture.ok_or_else(|| failed("пустой результат".to_owned()))
}
