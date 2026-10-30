//! Video frames from Windows Graphics Capture: no cursor, no yellow border where the system
//! allows turning it off (Windows 11; on Windows 10 it is drawn on screen but not in frames).
//! An area is a capture of its monitor cut on the GPU.

use std::ops::ControlFlow;

use windows::Win32::Foundation::HWND;
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

use super::frame::{self, Crop, CropArea, VideoFrame};
use super::monitors;
use crate::error::CaptureError;
use crate::list::area_on_monitor;
use crate::source::Source;

/// The least time between two frames asked of WGC: below any refresh period, so it never
/// holds a frame back.
const MIN_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

/// Receives frames on the capture thread.
pub trait FrameSink: Send + 'static {
    /// A new frame; [`ControlFlow::Break`] or an error ends the capture.
    fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<ControlFlow<()>, CaptureError>;

    /// The captured window was closed; no more frames will come.
    fn closed(&mut self);
}

/// A running capture on its own thread. Frames come only when the picture changes, at most at
/// the monitor refresh rate.
pub struct VideoCapture<S: FrameSink> {
    control: CaptureControl<Handler<S>, CaptureError>,
}

/// What the capture is pointed at.
enum Target {
    Monitor(Monitor),
    Window(Window),
}

impl Target {
    fn monitor(index: u32) -> Result<Self, CaptureError> {
        Ok(Self::Monitor(monitors::by_index(index)?))
    }

    fn window(id: u64) -> Result<Self, CaptureError> {
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
    /// Starts capturing `source`; frames go to `sink` on the capture thread.
    pub fn start(source: &Source, sink: S) -> Result<Self, CaptureError> {
        match *source {
            Source::Monitor(index) => Self::start_target(Target::monitor(index)?, None, sink),
            Source::Window(id) => Self::start_target(Target::window(id)?, None, sink),
            Source::Area(area) => {
                let monitors = monitors::list()?;
                let (monitor, local) = area_on_monitor(&monitors, area)?;
                let target = Target::monitor(monitor.index)?;
                let crop = CropArea {
                    local,
                    desktop: area,
                };
                Self::start_target(target, Some(crop), sink)
            }
        }
    }

    fn start_target(target: Target, crop: Option<CropArea>, sink: S) -> Result<Self, CaptureError> {
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
        // The default minimum interval is about 1/60 s, and frames come on the ticks of the
        // primary monitor: with 144 Hz there the next tick after it is the third (20.8 ms), so a
        // 60 fps picture gave 48 fps. Every composed frame is taken; `FrameGrid` drops extras.
        let interval =
            if GraphicsCaptureApi::is_minimum_update_interval_supported().map_err(|e| {
                CaptureError::platform("проверить возможности захвата")(
                    e.to_string(),
                )
            })? {
                MinimumUpdateIntervalSettings::Custom(MIN_UPDATE_INTERVAL)
            } else {
                MinimumUpdateIntervalSettings::Default
            };
        let settings = Settings::new(
            target,
            CursorCaptureSettings::WithoutCursor,
            border,
            SecondaryWindowSettings::Default,
            interval,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            Job { sink, crop },
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

/// What the capture thread gets at start.
struct Job<S> {
    sink: S,
    crop: Option<CropArea>,
}

struct Handler<S> {
    sink: S,
    crop: Option<Crop>,
}

impl<S: FrameSink> GraphicsCaptureApiHandler for Handler<S> {
    type Flags = Job<S>;
    type Error = CaptureError;

    fn new(context: Context<Job<S>>) -> Result<Self, CaptureError> {
        let Job { sink, crop } = context.flags;
        let crop = crop
            .map(|area| Crop::new(area, &context.device))
            .transpose()?;
        Ok(Self { sink, crop })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame<'_>,
        control: InternalCaptureControl,
    ) -> Result<(), CaptureError> {
        let view = frame::view(frame, self.crop.as_ref())?;
        if self.sink.frame(&view)?.is_break() {
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
