//! Windows: Windows Graphics Capture through windows-capture 2, Win32 through `windows`.

mod capture;
mod clock;
mod d3d;
mod frame;
mod monitors;
mod preview;
mod window_list;

pub use capture::{FrameSink, VideoCapture};
pub use clock::now;
pub use frame::VideoFrame;
pub use preview::{preview_monitor, preview_window};

use crate::error::CaptureError;
use crate::list::SourceList;

/// Monitors in system order (numbered from 1) and the windows a user would pick.
pub fn list_sources() -> Result<SourceList, CaptureError> {
    Ok(SourceList {
        monitors: monitors::list()?,
        windows: window_list::list()?,
    })
}
