//! Playback in the editor: the clock that moves the playhead (`clock.rs`), the sound thread
//! (`sound.rs`) and the jumps (`seek.rs`). While playing, the editor asks the preview for the
//! frame at the clock every frame; the preview's render thread takes only the newest request, so
//! late frames are dropped instead of queued. When the project sounds, the clock is the output
//! device's position; without sound, without a device or after the sound failed — the wall
//! clock. Playback stops at the end of the project.

mod clock;
pub mod seek;
mod sound;

use std::sync::Arc;
use std::time::Instant;

use dc_audio::Off;
use dc_core::{OutputTime, Project};
use dc_project::ProjectDir;
use dc_ui_kit::strings::editor as text;

use clock::{Clock, Pace};
use sound::{Sound, Start};

#[derive(Debug)]
pub struct Playback {
    clock: Clock,
    /// None until started (tests play by the wall clock).
    sound: Option<Sound>,
    project: Arc<Project>,
    revision: u64,
    /// Volume of the sound, 0..1.
    volume: f32,
}

impl Playback {
    pub fn new(project: Arc<Project>, revision: u64) -> Self {
        Self {
            clock: Clock::default(),
            sound: None,
            project,
            revision,
            volume: 1.0,
        }
    }

    /// Starts the sound thread of the project in `dir`.
    pub fn start(&mut self, dir: ProjectDir) -> std::io::Result<()> {
        self.sound = Some(Sound::start(dir)?);
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.clock.is_playing()
    }

    /// The project changed: while playing, the sound goes on from the playhead with it.
    pub fn changed(
        &mut self,
        project: Arc<Project>,
        revision: u64,
        now: Instant,
    ) -> Option<String> {
        self.project = project;
        self.revision = revision;
        if !self.is_playing() {
            return None;
        }
        let at = self.pause(now);
        self.play(at, now)
    }

    /// Plays from `at`; the message of a sound that cannot start (the picture plays anyway).
    pub fn play(&mut self, at: OutputTime, now: Instant) -> Option<String> {
        self.close();
        let Some(sound) = &self.sound else {
            self.clock.play(at, Pace::Wall(now));
            return None;
        };
        let line = Arc::new(dc_audio::Line::new(self.volume));
        let start = Start {
            project: Arc::clone(&self.project),
            revision: self.revision,
            from: at,
            line: Arc::clone(&line),
        };
        if sound.play(start) {
            self.clock.play(at, Pace::Sound(line));
            None
        } else {
            self.clock.play(at, Pace::Wall(now));
            Some(text::SOUND_STOPPED.to_owned())
        }
    }

    /// Stops; returns the playhead.
    pub fn pause(&mut self, now: Instant) -> OutputTime {
        self.close();
        self.clock.pause(now)
    }

    /// The playhead at `now` while playing, at `volume` (0..1); stops at `end`. Also the message
    /// when the sound failed and the picture went on by the wall clock.
    pub fn tick(&mut self, now: Instant, end: OutputTime, volume: f32) -> Tick {
        self.volume = volume;
        let mut tick = Tick::default();
        if !self.is_playing() {
            return tick;
        }
        if let Some(line) = self.clock.line() {
            line.set_volume(volume);
            if let Some(off) = line.off() {
                if let Off::Failed(error) = off {
                    tick.error = Some(text::sound_failed(&error));
                }
                self.clock.go_by_wall(now);
            }
        }
        let finished = self.clock.line().is_some_and(|line| line.finished(now));
        let at = self.clock.time(now);
        tick.at = Some(if finished || at >= end {
            self.close();
            self.clock = Clock::paused(end);
            end
        } else {
            at
        });
        tick
    }

    fn close(&self) {
        if let Some(line) = self.clock.line() {
            line.close();
        }
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.close();
    }
}

/// What a frame of playback changed.
#[derive(Debug, Default, PartialEq)]
pub struct Tick {
    /// The playhead, while playing (and the end, on the frame playback stopped there).
    pub at: Option<OutputTime>,
    /// Why the sound stopped, for a toast.
    pub error: Option<String>,
}
