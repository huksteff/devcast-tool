//! FFmpeg decoding to NV12 in system memory, optionally with D3D11VA hardware decoding,
//! and frame-accurate seeking (seek to the previous keyframe, decode forward to the target).

use std::ptr;

use ffmpeg::ffi;
use ffmpeg_next as ffmpeg;

/// One decoded frame: NV12 planes copied out tightly packed.
pub struct Nv12Frame {
    pub width: u32,
    pub height: u32,
    /// Presentation time in seconds.
    pub pts: f64,
    pub y: Vec<u8>,
    pub uv: Vec<u8>,
}

pub struct VideoDecoder {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    stream_index: usize,
    time_base: f64,
    pub fps: f64,
    pub duration: f64,
    scaler: Option<ffmpeg::software::scaling::Context>,
}

impl VideoDecoder {
    pub fn open(path: &str, hardware: bool) -> Result<Self, ffmpeg::Error> {
        let input = ffmpeg::format::input(path)?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let stream_index = stream.index();
        let time_base = f64::from(stream.time_base());
        let fps = f64::from(stream.avg_frame_rate());
        let duration = input.duration() as f64 / f64::from(ffi::AV_TIME_BASE);
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        if hardware {
            attach_d3d11va(&mut context)?;
        } else {
            context.set_threading(ffmpeg::threading::Config {
                kind: ffmpeg::threading::Type::Frame,
                count: 0,
            });
        }
        let decoder = context.decoder().video()?;
        Ok(Self { input, decoder, stream_index, time_base, fps, duration, scaler: None })
    }

    /// Next frame in presentation order, or None at end of stream.
    pub fn next_frame(&mut self) -> Result<Option<Nv12Frame>, ffmpeg::Error> {
        let mut frame = ffmpeg::frame::Video::empty();
        if self.next_raw(&mut frame)? {
            self.to_nv12(&frame).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Decodes the next frame into `frame` without copying it out. False at end of stream.
    fn next_raw(&mut self, frame: &mut ffmpeg::frame::Video) -> Result<bool, ffmpeg::Error> {
        loop {
            match self.decoder.receive_frame(frame) {
                Ok(()) => return Ok(true),
                Err(ffmpeg::Error::Other { errno: ffmpeg::error::EAGAIN }) => {}
                Err(ffmpeg::Error::Eof) => return Ok(false),
                Err(e) => return Err(e),
            }
            let mut fed = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.stream_index {
                    self.decoder.send_packet(&packet)?;
                    fed = true;
                    break;
                }
            }
            if !fed {
                self.decoder.send_eof()?;
            }
        }
    }

    /// Returns exactly the frame displayed at `target` seconds.
    pub fn seek(&mut self, target: f64) -> Result<Option<Nv12Frame>, ffmpeg::Error> {
        let ts = (target / self.time_base) as i64;
        // SAFETY: valid open input; seeks this stream backwards to the keyframe at or before ts.
        let rc = unsafe {
            ffi::av_seek_frame(
                self.input.as_mut_ptr(),
                self.stream_index as i32,
                ts,
                ffi::AVSEEK_FLAG_BACKWARD,
            )
        };
        if rc < 0 {
            return Err(ffmpeg::Error::from(rc));
        }
        self.decoder.flush();
        let half_frame = 0.5 / self.fps;
        let mut frame = ffmpeg::frame::Video::empty();
        // Intermediate frames are only decoded; just the target is copied out.
        while self.next_raw(&mut frame)? {
            let pts = frame.pts().unwrap_or(0) as f64 * self.time_base;
            if pts + half_frame >= target {
                return self.to_nv12(&frame).map(Some);
            }
        }
        Ok(None)
    }

    fn to_nv12(&mut self, frame: &ffmpeg::frame::Video) -> Result<Nv12Frame, ffmpeg::Error> {
        let pts = frame.pts().unwrap_or(0) as f64 * self.time_base;
        let mut downloaded = ffmpeg::frame::Video::empty();
        let source = if frame.format() == ffmpeg::format::Pixel::D3D11 {
            // SAFETY: frame is a D3D11 hardware frame from our decoder; `downloaded` is empty.
            let rc = unsafe { ffi::av_hwframe_transfer_data(downloaded.as_mut_ptr(), frame.as_ptr(), 0) };
            if rc < 0 {
                return Err(ffmpeg::Error::from(rc));
            }
            &downloaded
        } else {
            frame
        };
        let mut converted = ffmpeg::frame::Video::empty();
        let nv12 = if source.format() == ffmpeg::format::Pixel::NV12 {
            source
        } else {
            if self.scaler.is_none() {
                self.scaler = Some(ffmpeg::software::scaling::Context::get(
                    source.format(),
                    source.width(),
                    source.height(),
                    ffmpeg::format::Pixel::NV12,
                    source.width(),
                    source.height(),
                    ffmpeg::software::scaling::Flags::POINT,
                )?);
            }
            if let Some(scaler) = &mut self.scaler {
                scaler.run(source, &mut converted)?;
            }
            &converted
        };
        let (w, h) = (nv12.width() as usize, nv12.height() as usize);
        Ok(Nv12Frame {
            width: nv12.width(),
            height: nv12.height(),
            pts,
            y: packed_plane(nv12, 0, w, h),
            uv: packed_plane(nv12, 1, w, h / 2),
        })
    }
}

fn packed_plane(frame: &ffmpeg::frame::Video, plane: usize, row_bytes: usize, rows: usize) -> Vec<u8> {
    let stride = frame.stride(plane);
    let data = frame.data(plane);
    let mut out = Vec::with_capacity(row_bytes * rows);
    for row in 0..rows {
        out.extend_from_slice(&data[row * stride..row * stride + row_bytes]);
    }
    out
}

fn attach_d3d11va(context: &mut ffmpeg::codec::context::Context) -> Result<(), ffmpeg::Error> {
    let mut device: *mut ffi::AVBufferRef = ptr::null_mut();
    // SAFETY: creates a D3D11VA device owned by `device`; the codec context keeps its own reference.
    unsafe {
        let rc = ffi::av_hwdevice_ctx_create(
            &mut device,
            ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA,
            ptr::null(),
            ptr::null_mut(),
            0,
        );
        if rc < 0 {
            return Err(ffmpeg::Error::from(rc));
        }
        (*context.as_mut_ptr()).hw_device_ctx = ffi::av_buffer_ref(device);
        ffi::av_buffer_unref(&mut device);
    }
    Ok(())
}
