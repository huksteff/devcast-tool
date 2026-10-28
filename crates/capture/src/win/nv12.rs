//! BGRA capture frames → NV12 BT.709 limited range on the GPU (D3D11 video processor), then
//! one copy of the ready planes to memory (decision 006).
//!
//! The output size is fixed when recording starts. A frame of another size (a resized window)
//! is not scaled: it is placed at the top-left corner, cut off or padded with black.

use dc_encode::Nv12Frame;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING, D3D11_VIDEO_COLOR, D3D11_VIDEO_COLOR_0, D3D11_VIDEO_COLOR_RGBA,
    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0,
    D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_OPTIMAL_SPEED, D3D11_VPIV_DIMENSION_TEXTURE2D,
    D3D11_VPOV_DIMENSION_TEXTURE2D, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    ID3D11VideoContext1, ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator,
    ID3D11VideoProcessorInputView, ID3D11VideoProcessorOutputView,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
    DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::core::Interface;

use super::frame::VideoFrame;
use crate::error::CaptureError;

pub struct Nv12Converter {
    context: ID3D11DeviceContext,
    video_device: ID3D11VideoDevice,
    video_context: ID3D11VideoContext1,
    size: (u32, u32),
    output: ID3D11Texture2D,
    staging: ID3D11Texture2D,
    processor: Option<Processor>,
}

/// A video processor set up for one input size.
struct Processor {
    input: (u32, u32),
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    view: ID3D11VideoProcessorOutputView,
}

fn failed(action: &'static str) -> impl Fn(windows::core::Error) -> CaptureError {
    move |e| CaptureError::platform(action)(e.to_string())
}

/// The largest even size not bigger than the frame: NV12 halves color in both directions.
pub fn even_size((width, height): (u32, u32)) -> (u32, u32) {
    (width & !1, height & !1)
}

impl Nv12Converter {
    /// Converts frames of `frame`'s device into NV12 of `size` (even).
    pub fn new(frame: &VideoFrame<'_>, size: (u32, u32)) -> Result<Self, CaptureError> {
        let device = frame.device().clone();
        let context = frame.context().clone();
        let video_device: ID3D11VideoDevice = device
            .cast()
            .map_err(failed("включить видеопроцессор видеокарты"))?;
        let video_context: ID3D11VideoContext1 = context
            .cast()
            .map_err(failed("включить видеопроцессор видеокарты"))?;
        let output = texture(
            &device,
            size,
            D3D11_USAGE_DEFAULT,
            D3D11_BIND_RENDER_TARGET.0,
            0,
        )?;
        let staging = texture(
            &device,
            size,
            D3D11_USAGE_STAGING,
            0,
            D3D11_CPU_ACCESS_READ.0,
        )?;
        Ok(Self {
            context,
            video_device,
            video_context,
            size,
            output,
            staging,
            processor: None,
        })
    }

    pub fn convert(
        &mut self,
        frame: &VideoFrame<'_>,
        out: &mut Nv12Frame,
    ) -> Result<(), CaptureError> {
        let input = frame.size();
        let p = match self.processor.take() {
            Some(p) if p.input == input => p,
            _ => self.processor(input)?,
        };
        let view = input_view(&self.video_device, &p.enumerator, frame.texture())?;
        let (width, height) = (input.0.min(self.size.0), input.1.min(self.size.1));
        let rect = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        let stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: std::mem::ManuallyDrop::new(Some(view)),
            ..Default::default()
        };
        let streams = [stream];
        // SAFETY: the processor, views and rectangles belong to this device; `rect` lies inside
        // both the input and the output.
        let blit = unsafe {
            self.video_context.VideoProcessorSetStreamSourceRect(
                &p.processor,
                0,
                true,
                Some(&raw const rect),
            );
            self.video_context.VideoProcessorSetStreamDestRect(
                &p.processor,
                0,
                true,
                Some(&raw const rect),
            );
            self.video_context
                .VideoProcessorBlt(&p.processor, &p.view, 0, &streams)
        };
        let [stream] = streams;
        drop(std::mem::ManuallyDrop::into_inner(stream.pInputSurface));
        self.processor = Some(p);
        blit.map_err(failed("перевести кадр в NV12"))?;
        self.read(out)
    }

    /// Copies the NV12 texture to `out` through the staging texture.
    fn read(&self, out: &mut Nv12Frame) -> Result<(), CaptureError> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: same size and format; the mapping covers RowPitch × Height rows of luma
        // followed by Height / 2 rows of color (NV12 layout of a staging texture) and is
        // released before returning.
        unsafe {
            self.context.CopyResource(&self.staging, &self.output);
            self.context
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&raw mut mapped))
                .map_err(failed("прочитать кадр из видеопамяти"))?;
            let pitch = mapped.RowPitch as usize;
            let height = self.size.1 as usize;
            let all = std::slice::from_raw_parts(mapped.pData.cast::<u8>(), pitch * height * 3 / 2);
            let (y, uv) = all.split_at(pitch * height);
            out.copy_from(y, uv, pitch);
            self.context.Unmap(&self.staging, 0);
        }
        Ok(())
    }

    fn processor(&self, input: (u32, u32)) -> Result<Processor, CaptureError> {
        let rate = DXGI_RATIONAL {
            Numerator: 60,
            Denominator: 1,
        };
        let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: input.0,
            InputHeight: input.1,
            OutputFrameRate: rate,
            OutputWidth: self.size.0,
            OutputHeight: self.size.1,
            Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
        };
        let failed = failed("настроить видеопроцессор видеокарты");
        let view_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };
        let black = D3D11_VIDEO_COLOR {
            Anonymous: D3D11_VIDEO_COLOR_0 {
                RGBA: D3D11_VIDEO_COLOR_RGBA {
                    R: 0.0,
                    G: 0.0,
                    B: 0.0,
                    A: 1.0,
                },
            },
        };
        let mut view = None;
        // SAFETY: valid descriptions; created objects are owned by the returned value.
        unsafe {
            let enumerator = self
                .video_device
                .CreateVideoProcessorEnumerator(&raw const desc)
                .map_err(&failed)?;
            let processor = self
                .video_device
                .CreateVideoProcessor(&enumerator, 0)
                .map_err(&failed)?;
            self.video_device
                .CreateVideoProcessorOutputView(
                    &self.output,
                    &enumerator,
                    &raw const view_desc,
                    Some(&raw mut view),
                )
                .map_err(&failed)?;
            let vc = &self.video_context;
            vc.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            vc.VideoProcessorSetStreamColorSpace1(
                &processor,
                0,
                DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
            );
            vc.VideoProcessorSetOutputColorSpace1(
                &processor,
                DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            );
            vc.VideoProcessorSetOutputBackgroundColor(&processor, false, &raw const black);
            let view = view.ok_or_else(|| failed(windows::core::Error::empty()))?;
            Ok(Processor {
                input,
                enumerator,
                processor,
                view,
            })
        }
    }
}

fn input_view(
    video_device: &ID3D11VideoDevice,
    enumerator: &ID3D11VideoProcessorEnumerator,
    texture: &ID3D11Texture2D,
) -> Result<ID3D11VideoProcessorInputView, CaptureError> {
    let desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
        FourCC: 0,
        ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
        Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
            Texture2D: D3D11_TEX2D_VPIV {
                MipSlice: 0,
                ArraySlice: 0,
            },
        },
    };
    let failed = failed("передать кадр видеопроцессору");
    let mut view = None;
    // SAFETY: a texture of this device, a valid description and an out slot.
    unsafe {
        video_device.CreateVideoProcessorInputView(
            texture,
            enumerator,
            &raw const desc,
            Some(&raw mut view),
        )
    }
    .map_err(&failed)?;
    view.ok_or_else(|| failed(windows::core::Error::empty()))
}

fn texture(
    device: &ID3D11Device,
    (width, height): (u32, u32),
    usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE,
    bind: i32,
    cpu: i32,
) -> Result<ID3D11Texture2D, CaptureError> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: usage,
        BindFlags: bind as u32,
        CPUAccessFlags: cpu as u32,
        MiscFlags: 0,
    };
    let failed = failed("создать текстуру NV12");
    let mut texture = None;
    // SAFETY: a valid description and an out slot for the new texture.
    unsafe { device.CreateTexture2D(&raw const desc, None, Some(&raw mut texture)) }
        .map_err(&failed)?;
    texture.ok_or_else(|| failed(windows::core::Error::empty()))
}
