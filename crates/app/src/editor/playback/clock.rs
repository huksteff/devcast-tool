//! The playback clock: where the playhead is at a moment. While the project sounds, the time is
//! the start plus the frames the output device has played (`dc_audio::Line::heard`), so the
//! picture follows the sound; without sound — the wall clock (`Instant`).

use std::sync::Arc;
use std::time::Instant;

use dc_audio::{Line, RATE};
use dc_core::{OutputTime, TimeDelta};

/// What moves the playhead while playing.
#[derive(Debug, Clone)]
pub enum Pace {
    /// The wall clock since this moment.
    Wall(Instant),
    /// The frames the output device has played of this line.
    Sound(Arc<Line>),
}

#[derive(Debug, Clone)]
enum State {
    Paused(OutputTime),
    Playing { from: OutputTime, pace: Pace },
}

#[derive(Debug, Clone)]
pub struct Clock {
    state: State,
}

impl Default for Clock {
    fn default() -> Self {
        Self::paused(OutputTime::ZERO)
    }
}

impl Clock {
    pub fn paused(at: OutputTime) -> Self {
        Self {
            state: State::Paused(at),
        }
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.state, State::Playing { .. })
    }

    /// The line the clock follows, while it follows one.
    pub fn line(&self) -> Option<&Arc<Line>> {
        match &self.state {
            State::Playing {
                pace: Pace::Sound(line),
                ..
            } => Some(line),
            _ => None,
        }
    }

    /// The playhead at `now`.
    pub fn time(&self, now: Instant) -> OutputTime {
        match &self.state {
            State::Paused(at) => *at,
            State::Playing { from, pace } => *from + elapsed(pace, now),
        }
    }

    /// Plays from `at` by `pace`.
    pub fn play(&mut self, at: OutputTime, pace: Pace) {
        self.state = State::Playing { from: at, pace };
    }

    /// Stops at the playhead of `now`; returns it.
    pub fn pause(&mut self, now: Instant) -> OutputTime {
        let at = self.time(now);
        self.state = State::Paused(at);
        at
    }

    /// Goes on from the playhead of `now` by the wall clock: the sound gave up.
    pub fn go_by_wall(&mut self, now: Instant) {
        if self.is_playing() {
            let at = self.time(now);
            self.play(at, Pace::Wall(now));
        }
    }
}

fn elapsed(pace: &Pace, now: Instant) -> TimeDelta {
    match pace {
        Pace::Wall(start) => {
            TimeDelta::from_micros(now.saturating_duration_since(*start).as_micros() as i64)
        }
        Pace::Sound(line) => {
            let frames = i128::from(line.heard(now));
            TimeDelta::from_micros((frames * 1_000_000 / i128::from(RATE)) as i64)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn secs(s: f64) -> OutputTime {
        OutputTime::from_secs_f64(s)
    }

    #[test]
    fn the_wall_clock_runs_from_the_start_and_stops_on_pause() {
        let t0 = Instant::now();
        let mut clock = Clock::paused(secs(2.0));
        assert_eq!(clock.time(t0 + Duration::from_secs(5)), secs(2.0));
        clock.play(secs(2.0), Pace::Wall(t0));
        assert!(clock.is_playing());
        assert_eq!(clock.time(t0 + Duration::from_millis(1500)), secs(3.5));
        assert_eq!(clock.pause(t0 + Duration::from_secs(1)), secs(3.0));
        assert_eq!(clock.time(t0 + Duration::from_secs(9)), secs(3.0));
        let t1 = t0 + Duration::from_secs(20);
        clock.play(secs(3.0), Pace::Wall(t1));
        assert_eq!(
            clock.time(t1 + Duration::from_millis(250)),
            secs(3.25),
            "resumes where it stopped"
        );
    }

    #[test]
    fn the_sound_clock_follows_the_played_frames_not_the_wall() {
        let line = Arc::new(Line::new(1.0));
        let mut clock = Clock::default();
        clock.play(secs(10.0), Pace::Sound(Arc::clone(&line)));
        let t0 = Instant::now();
        assert_eq!(
            clock.time(t0 + Duration::from_secs(2)),
            secs(10.0),
            "no sound played: the picture waits"
        );
        line.pull(&mut [0.0; 960], t0);
        assert_eq!(
            clock.time(t0 + Duration::from_secs(2)),
            secs(10.0),
            "an empty queue is not played"
        );
        assert!(clock.line().is_some());
    }

    #[test]
    fn a_sound_that_gave_up_hands_over_to_the_wall_clock() {
        let line = Arc::new(Line::new(1.0));
        let mut clock = Clock::default();
        clock.play(secs(4.0), Pace::Sound(line));
        let t0 = Instant::now();
        clock.go_by_wall(t0);
        assert!(clock.line().is_none());
        assert_eq!(clock.time(t0 + Duration::from_millis(500)), secs(4.5));
        let mut paused = Clock::paused(secs(1.0));
        paused.go_by_wall(t0);
        assert!(!paused.is_playing(), "a paused clock stays paused");
    }

    #[test]
    fn seeking_while_playing_restarts_from_the_new_moment() {
        let t0 = Instant::now();
        let mut clock = Clock::default();
        clock.play(secs(0.0), Pace::Wall(t0));
        let t1 = t0 + Duration::from_secs(3);
        clock.play(secs(30.0), Pace::Wall(t1));
        assert_eq!(clock.time(t1 + Duration::from_secs(1)), secs(31.0));
    }
}
