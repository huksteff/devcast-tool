//! A single frame of a monitor or window, shrunk for the source picker.

use std::ops::ControlFlow;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::IsIconic;

use super::capture::{FrameSink, Target, VideoCapture, VideoFrame};
use crate::error::CaptureError;
use crate::preview::{BgraImage, Preview};

/// The first frame normally arrives within tens of milliseconds; this bound only turns a
/// capture that delivers nothing into an error instead of a hang.
const FIRST_FRAME: Duration = Duration::from_secs(2);

/// Monitor `index` (from 1), longer side at most `max_side` pixels.
pub fn preview_monitor(index: u32, max_side: u32) -> Result<Preview, CaptureError> {
    Ok(grab(Target::monitor(index)?)?.preview(max_side))
}

/// Window `id` (not minimized), longer side at most `max_side` pixels.
pub fn preview_window(id: u64, max_side: u32) -> Result<Preview, CaptureError> {
    let target = Target::window(id)?;
    // SAFETY: IsIconic accepts any handle and only checks it.
    if unsafe { IsIconic(HWND(id as *mut _)) }.as_bool() {
        return Err(CaptureError::WindowMinimized(id));
    }
    Ok(grab(target)?.preview(max_side))
}

struct FirstFrame(Sender<BgraImage>);

impl FrameSink for FirstFrame {
    fn frame(&mut self, frame: &VideoFrame<'_>) -> Result<ControlFlow<()>, CaptureError> {
        // The receiver is gone only if the caller stopped waiting; then the image is not needed.
        let _ = self.0.send(frame.to_bgra()?);
        Ok(ControlFlow::Break(()))
    }

    fn closed(&mut self) {}
}

fn grab(target: Target) -> Result<BgraImage, CaptureError> {
    let (sender, receiver) = mpsc::channel();
    let capture = VideoCapture::start(target, FirstFrame(sender))?;
    if let Ok(image) = receiver.recv_timeout(FIRST_FRAME) {
        capture.wait()?;
        return Ok(image);
    }
    capture.stop()?;
    Err(CaptureError::NoFrame {
        seconds: FIRST_FRAME.as_secs(),
    })
}
