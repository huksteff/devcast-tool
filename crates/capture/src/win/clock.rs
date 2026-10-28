//! The host clock on Windows: QueryPerformanceCounter.

use windows::Foundation::TimeSpan;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

use crate::error::CaptureError;
use crate::event::HostTime;

/// Now on the QPC timeline.
pub fn now() -> Result<HostTime, CaptureError> {
    let (mut counter, mut frequency) = (0i64, 0i64);
    // SAFETY: both calls write one i64 into the given place.
    unsafe {
        QueryPerformanceCounter(&raw mut counter)
            .and_then(|()| QueryPerformanceFrequency(&raw mut frequency))
    }
    .map_err(|e| {
        CaptureError::platform("прочитать системные часы")(e.to_string())
    })?;
    let micros = i128::from(counter) * 1_000_000 / i128::from(frequency);
    Ok(HostTime::from_micros(micros as i64))
}

/// A WinRT `SystemRelativeTime` (QPC in 100 ns units, as on capture frames).
pub fn from_system_relative(time: TimeSpan) -> HostTime {
    HostTime::from_micros(time.Duration / 10)
}
