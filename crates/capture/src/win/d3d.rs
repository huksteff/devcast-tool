//! Reading a GPU texture back into memory.

use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};

use crate::error::CaptureError;
use crate::preview::BgraImage;

/// Copies a BGRA8 texture to the CPU through a staging texture.
pub fn read_bgra(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    texture: &ID3D11Texture2D,
) -> Result<BgraImage, CaptureError> {
    let failed = |e: windows::core::Error| {
        CaptureError::platform("прочитать кадр из видеопамяти")(
            e.to_string(),
        )
    };
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: GetDesc fills the struct it is given.
    unsafe { texture.GetDesc(&raw mut desc) };
    let staging_desc = D3D11_TEXTURE2D_DESC {
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
        ..desc
    };
    let mut staging = None;
    // SAFETY: a valid description and an out slot for the new texture.
    unsafe { device.CreateTexture2D(&raw const staging_desc, None, Some(&raw mut staging)) }
        .map_err(failed)?;
    let staging = staging.ok_or_else(|| failed(windows::core::Error::empty()))?;
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: both textures have the same size and format; the mapping is released below
    // before `staging` is dropped, and rows are read within RowPitch × Height.
    unsafe {
        context.CopyResource(&staging, texture);
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&raw mut mapped))
            .map_err(failed)?;
        let (width, height) = (desc.Width as usize, desc.Height as usize);
        let pitch = mapped.RowPitch as usize;
        let rows = std::slice::from_raw_parts(mapped.pData.cast::<u8>(), pitch * height);
        let mut bgra = Vec::with_capacity(width * height * 4);
        for row in rows.chunks_exact(pitch) {
            bgra.extend_from_slice(&row[..width * 4]);
        }
        context.Unmap(&staging, 0);
        Ok(BgraImage {
            width: desc.Width,
            height: desc.Height,
            bgra,
        })
    }
}
