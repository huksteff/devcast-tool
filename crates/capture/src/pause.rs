//! Pauses of a recording and the recording clock: host time with the pauses cut out.
//!
//! A pause is cut out of every track by time, not by when a packet arrives: whatever was
//! captured inside a pause is dropped, and everything after it is moved earlier by the length
//! of the pauses before it. Every file of the recording (segments, `cursor.bin`, `keys.bin`)
//! is on this clock, so tick `n` of a track is at `start + n / rate` on it; the manifest lists
//! the pauses on the host clock.

use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::event::HostTime;

/// A pause on the host clock. `resumed: None` — the recording is paused now (or was stopped
/// while paused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pause {
    pub at: HostTime,
    pub resumed: Option<HostTime>,
}

/// Where a host time lands on the recording clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// The recording clock; a time inside a pause gets the moment the pause began.
    pub time: HostTime,
    /// The time is inside a pause: what was captured then is not recorded.
    pub paused: bool,
}

/// The pauses of a recording so far, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pauses(Vec<Pause>);

impl Pauses {
    pub fn list(&self) -> &[Pause] {
        &self.0
    }

    pub fn is_paused(&self) -> bool {
        self.0.last().is_some_and(|p| p.resumed.is_none())
    }

    pub fn pause(&mut self, at: HostTime) -> Result<(), CaptureError> {
        if self.is_paused() {
            return Err(CaptureError::AlreadyPaused);
        }
        self.0.push(Pause { at, resumed: None });
        Ok(())
    }

    pub fn resume(&mut self, at: HostTime) -> Result<(), CaptureError> {
        match self.0.last_mut() {
            Some(pause) if pause.resumed.is_none() => {
                pause.resumed = Some(at.max(pause.at));
                Ok(())
            }
            _ => Err(CaptureError::NotPaused),
        }
    }

    pub fn place(&self, time: HostTime) -> Placed {
        let mut cut = 0;
        for pause in &self.0 {
            if time < pause.at {
                break;
            }
            match pause.resumed {
                Some(resumed) if time >= resumed => cut += resumed.micros() - pause.at.micros(),
                _ => {
                    return Placed {
                        time: HostTime::from_micros(pause.at.micros() - cut),
                        paused: true,
                    };
                }
            }
        }
        Placed {
            time: HostTime::from_micros(time.micros() - cut),
            paused: false,
        }
    }

    /// The end of the first pause that ends after `time`, if it ended by `until`: a picture
    /// captured at `time` during a pause is what the screen shows when the recording goes on.
    pub fn resumed_after(&self, time: HostTime, until: HostTime) -> Option<HostTime> {
        self.0
            .iter()
            .filter_map(|p| p.resumed)
            .find(|resumed| *resumed > time)
            .filter(|resumed| *resumed <= until)
    }
}

/// [`Pauses`] shared by the threads of a recording: the control thread changes them, every
/// track reads them.
#[derive(Debug, Clone, Default)]
pub struct PauseClock(Arc<Mutex<Pauses>>);

impl PauseClock {
    /// Pauses at `at`; returns the pauses for the manifest.
    pub fn pause(&self, at: HostTime) -> Result<Pauses, CaptureError> {
        let mut pauses = self.lock()?;
        pauses.pause(at)?;
        Ok(pauses.clone())
    }

    pub fn resume(&self, at: HostTime) -> Result<Pauses, CaptureError> {
        let mut pauses = self.lock()?;
        pauses.resume(at)?;
        Ok(pauses.clone())
    }

    pub fn place(&self, time: HostTime) -> Result<Placed, CaptureError> {
        Ok(self.lock()?.place(time))
    }

    pub fn resumed_after(
        &self,
        time: HostTime,
        until: HostTime,
    ) -> Result<Option<HostTime>, CaptureError> {
        Ok(self.lock()?.resumed_after(time, until))
    }

    pub fn is_paused(&self) -> Result<bool, CaptureError> {
        Ok(self.lock()?.is_paused())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Pauses>, CaptureError> {
        self.0.lock().map_err(|_| {
            CaptureError::platform("поставить запись на паузу")(
                "поток управления записью упал".to_owned(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: i64) -> HostTime {
        HostTime::from_micros(1_000_000_000 + ms * 1000)
    }

    fn placed(ms: i64, paused: bool) -> Placed {
        Placed {
            time: at(ms),
            paused,
        }
    }

    #[test]
    fn pauses_are_cut_out_of_the_clock() {
        let mut pauses = Pauses::default();
        assert_eq!(pauses.place(at(500)), placed(500, false));
        pauses.pause(at(1000)).unwrap();
        assert!(pauses.is_paused());
        assert_eq!(pauses.place(at(999)), placed(999, false));
        assert_eq!(pauses.place(at(1000)), placed(1000, true));
        assert_eq!(pauses.place(at(9000)), placed(1000, true));
        pauses.resume(at(3000)).unwrap();
        assert!(!pauses.is_paused());
        assert_eq!(pauses.place(at(2999)), placed(1000, true));
        assert_eq!(pauses.place(at(3000)), placed(1000, false));
        assert_eq!(pauses.place(at(3500)), placed(1500, false));
        pauses.pause(at(4000)).unwrap();
        pauses.resume(at(4250)).unwrap();
        assert_eq!(pauses.place(at(4100)), placed(2000, true));
        assert_eq!(pauses.place(at(5000)), placed(2750, false));
        assert_eq!(
            pauses.list(),
            [
                Pause {
                    at: at(1000),
                    resumed: Some(at(3000))
                },
                Pause {
                    at: at(4000),
                    resumed: Some(at(4250))
                }
            ]
        );
    }

    #[test]
    fn pause_and_resume_alternate() {
        let mut pauses = Pauses::default();
        assert!(matches!(pauses.resume(at(0)), Err(CaptureError::NotPaused)));
        pauses.pause(at(0)).unwrap();
        assert!(matches!(
            pauses.pause(at(1)),
            Err(CaptureError::AlreadyPaused)
        ));
        pauses.resume(at(2)).unwrap();
        assert!(matches!(pauses.resume(at(3)), Err(CaptureError::NotPaused)));
    }

    #[test]
    fn picture_of_a_pause_shows_from_its_end() {
        let mut pauses = Pauses::default();
        pauses.pause(at(1000)).unwrap();
        assert_eq!(pauses.resumed_after(at(1500), at(9000)), None);
        pauses.resume(at(2000)).unwrap();
        pauses.pause(at(3000)).unwrap();
        pauses.resume(at(4000)).unwrap();
        assert_eq!(pauses.resumed_after(at(1500), at(1999)), None);
        assert_eq!(pauses.resumed_after(at(1500), at(2000)), Some(at(2000)));
        assert_eq!(pauses.resumed_after(at(3500), at(9000)), Some(at(4000)));
        assert_eq!(pauses.resumed_after(at(4500), at(9000)), None);
    }
}
