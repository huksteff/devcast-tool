//! The sound thread of playback, «devcast-sound»: the mix of the export (`dc_ops::sound::levels`
//! and `dc_audio::Mixer`: tracks, levels, mute, extra sound) fed from the playhead into a line
//! that the default output device plays. The mixer is built once per project revision and its
//! loudness measurements are kept for the session. While paused the thread waits on its channel
//! and nothing runs.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use dc_audio::{Line, LoudnessCache, Mixer, Off, Output, feed, frame_at};
use dc_core::{OutputTime, Project};
use dc_project::ProjectDir;

/// Play `project` of `revision` from `from` into `line`; closing the line stops it.
pub struct Start {
    pub project: Arc<Project>,
    pub revision: u64,
    pub from: OutputTime,
    pub line: Arc<Line>,
}

#[derive(Debug)]
pub struct Sound {
    starts: Sender<Start>,
    /// Last: joined after the channel closes (`Joined`).
    _thread: crate::editor::threads::Joined,
}

impl Sound {
    /// Starts the thread of the project in `dir`; it ends when this is dropped (the line playing
    /// is closed first by its owner).
    pub fn start(dir: ProjectDir) -> std::io::Result<Self> {
        let (starts, inbox) = channel();
        let thread = std::thread::Builder::new()
            .name("devcast-sound".to_owned())
            .spawn(move || serve(&dir, &inbox))?;
        Ok(Self {
            starts,
            _thread: crate::editor::threads::Joined::new(thread),
        })
    }

    /// Asks to play; `false` if the thread is gone.
    pub fn play(&self, start: Start) -> bool {
        self.starts.send(start).is_ok()
    }
}

/// The mixer of the last revision played and what it measured.
struct Mix {
    mixer: Option<(u64, Mixer)>,
    loudness: LoudnessCache,
}

fn serve(dir: &ProjectDir, inbox: &Receiver<Start>) {
    let mut mix = Mix {
        mixer: None,
        loudness: LoudnessCache::default(),
    };
    while let Ok(mut start) = inbox.recv() {
        while let Ok(newer) = inbox.try_recv() {
            start.line.close();
            start = newer;
        }
        if let Err(error) = play(dir, &mut mix, &start) {
            start.line.turn_off(Off::Failed(error));
        }
    }
}

/// Plays `start` until its line is closed; an error is the message for the user.
fn play(dir: &ProjectDir, mix: &mut Mix, start: &Start) -> Result<(), String> {
    if start.line.is_closed() {
        return Ok(());
    }
    let mixer = match &mut mix.mixer {
        Some((revision, mixer)) if *revision == start.revision => mixer,
        slot => {
            *slot = None;
            let levels =
                dc_ops::sound::levels(dir.root(), &start.project).map_err(|e| e.to_string())?;
            let mixer = Mixer::new(dir, &start.project, &levels, &mut mix.loudness)
                .map_err(|e| e.to_string())?;
            &mut slot.insert((start.revision, mixer)).1
        }
    };
    if mixer.is_silent() {
        start.line.turn_off(Off::Silent);
        return Ok(());
    }
    let _output = Output::open(Arc::clone(&start.line)).map_err(|e| e.to_string())?;
    feed(mixer, &start.line, frame_at(start.from.micros())).map_err(|e| e.to_string())?;
    // The end of the project is queued: the device plays the rest until the line is closed.
    start.line.wait_closed();
    Ok(())
}
