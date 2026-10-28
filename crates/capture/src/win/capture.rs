//! Video frames from Windows Graphics Capture: no cursor, no yellow border where the system
//! allows turning it off (Windows 11; on Windows 10 it is drawn on screen but not in frames).

use std::ops::ControlFlow;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::UI::WindowsAndMessaging::IsWindow;
use windows_capture::capture::{
    CaptureControl, CaptureControlError, Context, GraphicsCaptureApiError,
    GraphicsCaptureApiHandler,
};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    GraphicsCaptureItemType, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use super::{d3d, monitors};
use crate::error::CaptureError;
use crate::preview::BgraImage;

/// Receives frames on the capture thread.
pub trait FrameSink: Send + 'static {
    /// A new frame; [`ControlFlow::Break`] or an error ends the capture.
    fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<ControlFlow<()>, CaptureError>;

    /// The captured window was closed; no more frames will come.
    fn closed(&mut self);
}

/// One captured frame, valid only inside [`FrameSink::frame`].
pub struct VideoFrame<'a> {
    frame: &'a Frame<'a>,
}

impl VideoFrame<'_> {
    /// The frame on the GPU, BGRA8.
    pub fn texture(&self) -> &ID3D11Texture2D {
        self.frame.as_raw_texture()
    }

    pub fn size(&self) -> (u32, u32) {
        (self.frame.width(), self.frame.height())
    }

    /// Copies the frame to memory (slow: for previews and checks, not for every frame).
    pub fn to_bgra(&self) -> Result<BgraImage, CaptureError> {
        d3d::read_bgra(
            self.frame.device(),
            self.frame.device_context(),
            self.texture(),
        )
    }
}

/// A running capture on its own thread.
pub struct VideoCapture<S: FrameSink> {
    control: CaptureControl<Handler<S>, CaptureError>,
}

/// What the capture is pointed at.
pub(crate) enum Target {
    Monitor(Monitor),
    Window(Window),
}

impl Target {
    pub(crate) fn monitor(index: u32) -> Result<Self, CaptureError> {
        Ok(Self::Monitor(monitors::by_index(index)?))
    }

    pub(crate) fn window(id: u64) -> Result<Self, CaptureError> {
        let hwnd = HWND(id as *mut _);
        // SAFETY: IsWindow accepts any value and only checks it.
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(CaptureError::WindowNotFound(id));
        }
        Ok(Self::Window(Window::from_raw_hwnd(hwnd.0)))
    }
}

impl TryFrom<Target> for GraphicsCaptureItemType {
    type Error = windows::core::Error;

    fn try_from(target: Target) -> Result<Self, Self::Error> {
        match target {
            Target::Monitor(monitor) => monitor.try_into(),
            Target::Window(window) => window.try_into(),
        }
    }
}

impl<S: FrameSink> VideoCapture<S> {
    pub(crate) fn start(target: Target, sink: S) -> Result<Self, CaptureError> {
        let supported = GraphicsCaptureApi::is_border_settings_supported().map_err(|e| {
            CaptureError::platform("проверить возможности захвата")(
                e.to_string(),
            )
        })?;
        let border = if supported {
            DrawBorderSettings::WithoutBorder
        } else {
            DrawBorderSettings::Default
        };
        let settings = Settings::new(
            target,
            CursorCaptureSettings::WithoutCursor,
            border,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            sink,
        );
        let control = Handler::start_free_threaded(settings).map_err(api_error)?;
        Ok(Self { control })
    }

    /// Stops the capture and returns the sink's error, if it failed.
    pub fn stop(self) -> Result<(), CaptureError> {
        self.control.stop().map_err(control_error)
    }

    /// Waits until the sink ends the capture or the window closes.
    pub fn wait(self) -> Result<(), CaptureError> {
        self.control.wait().map_err(control_error)
    }
}

struct Handler<S> {
    sink: S,
}

impl<S: FrameSink> GraphicsCaptureApiHandler for Handler<S> {
    type Flags = S;
    type Error = CaptureError;

    fn new(context: Context<S>) -> Result<Self, CaptureError> {
        Ok(Self {
            sink: context.flags,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame<'_>,
        control: InternalCaptureControl,
    ) -> Result<(), CaptureError> {
        if self.sink.frame(&VideoFrame { frame })?.is_break() {
            control.stop();
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), CaptureError> {
        self.sink.closed();
        Ok(())
    }
}

fn api_error(error: GraphicsCaptureApiError<CaptureError>) -> CaptureError {
    use windows_capture::graphics_capture_api::Error as Api;
    match error {
        GraphicsCaptureApiError::NewHandlerError(e)
        | GraphicsCaptureApiError::FrameHandlerError(e) => e,
        GraphicsCaptureApiError::GraphicsCaptureApiError(Api::CursorConfigUnsupported) => {
            CaptureError::CursorUnsupported
        }
        other => CaptureError::platform("запустить захват экрана")(
            other.to_string(),
        ),
    }
}

fn control_error(error: CaptureControlError<CaptureError>) -> CaptureError {
    match error {
        CaptureControlError::StoppedHandlerError(e) => e,
        CaptureControlError::GraphicsCaptureApiError(e) => api_error(e),
        other => CaptureError::platform("остановить захват экрана")(
            other.to_string(),
        ),
    }
}
