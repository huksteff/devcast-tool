//! Ctrl+C, Ctrl+Break and closing the console window of a recording process: instead of the
//! process being killed with an unfinished recording, the given stop runs and the recording is
//! finished as after a normal stop.

use std::sync::OnceLock;

use windows::Win32::Foundation::{FALSE, TRUE};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::core::BOOL;

use crate::error::CaptureError;

type Stop = Box<dyn Fn() + Send + Sync>;

static STOP: OnceLock<Stop> = OnceLock::new();

/// From now on these console events call `stop` on a system thread and wait for it; the
/// process then goes on (Ctrl+C, Ctrl+Break) or ends (closed window: Windows ends it once
/// `stop` returns, or after about 5 s — then the recording is recovered when the project is
/// opened). Once per process.
pub fn on_console_stop(stop: impl Fn() + Send + Sync + 'static) -> Result<(), CaptureError> {
    let failed = CaptureError::platform("перехватить Ctrl+C");
    STOP.set(Box::new(stop))
        .map_err(|_| failed("обработчик уже установлен".to_owned()))?;
    // SAFETY: `handler` is a plain function that lives as long as the process.
    unsafe { SetConsoleCtrlHandler(Some(handler), true) }
        .map_err(|e| CaptureError::platform("перехватить Ctrl+C")(e.to_string()))
}

unsafe extern "system" fn handler(event: u32) -> BOOL {
    match event {
        CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
            if let Some(stop) = STOP.get() {
                stop();
            }
            TRUE
        }
        _ => FALSE,
    }
}
