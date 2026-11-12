//! Windows: Windows Graphics Capture through windows-capture 2, Win32 through `windows`.

mod capture;
mod clock;
mod console;
mod cursor;
mod d3d;
mod disk;
mod frame;
mod hooks;
pub mod hud_window;
mod input;
mod keys;
mod monitors;
mod mouse;
mod nv12;
mod origin;
pub mod owner;
mod preview;
mod project;
mod recording;
mod video;
mod video_encode;
mod window_list;

pub use capture::{FrameSink, VideoCapture};
pub use clock::now;
pub use console::on_console_stop;
pub use disk::{ROOM_LOW, ROOM_TO_START, free_space, low_room, room_to_start};
pub use frame::VideoFrame;
pub use input::InputRecording;
pub use origin::{source_origin, window_frame};
pub use preview::{preview_monitor, preview_window};
pub use project::{ProjectRecording, RecordedProject};
pub use recording::{Recording, RecordingSummary};
pub use video::{VideoRecording, VideoStart, VideoSummary};

use crate::error::CaptureError;
use crate::list::SourceList;

/// Monitors in system order (numbered from 1) and the windows a user would pick.
pub fn list_sources() -> Result<SourceList, CaptureError> {
    Ok(SourceList {
        monitors: monitors::list()?,
        windows: window_list::list()?,
    })
}
