//! The presses of the recordings (7.15, 7.16): the button clicks and the shortcuts of each
//! recording's telemetry (`dc_capture::input::Telemetry::clicks` and `key_presses`, the same as
//! `devcast events clicks` and the keys effect read), read on the thread `devcast-presses`
//! (`reader.rs`). The «Курсор» row shows the dots of the clicks, the «Клавиши» row the shortcuts
//! once they come.

use dc_capture::input::{Click, Telemetry};
use dc_capture::project::recording_dir;
use dc_core::project::{Media, MediaKind};
use dc_core::{KeyPress, KeyTracks, Project};
use dc_project::ProjectDir;
use eframe::egui;

use super::reader::{Reader, Wanted};

/// What a recording's telemetry holds for the timeline, each list in time order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pressed {
    pub clicks: Vec<Click>,
    pub keys: Vec<KeyPress>,
}

/// The presses of each recording.
pub type Presses = Reader<Media, Pressed>;

impl Wanted for Media {
    const THREAD: &'static str = "devcast-presses";
    const FAILED: &'static str = "клики и клавиши записи не прочитаны";
}

impl Presses {
    /// Starts the thread that reads the presses of the project in `dir`; it wakes the main
    /// window of `ctx` after every answer.
    pub fn start(&mut self, dir: ProjectDir, ctx: egui::Context) -> std::io::Result<()> {
        let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.start_with(move |media| load(&dir, media), wake)
    }

    /// The shortcuts of the recordings of `project` read so far, as the keys effect takes them.
    pub fn key_tracks(&self, project: &Project) -> KeyTracks {
        recordings(project)
            .into_iter()
            .filter_map(|media| Some((media.path.clone(), self.get(&media)?.keys.clone())))
            .collect()
    }
}

/// The recordings of the track of `project`, once each: imported videos have no telemetry.
pub fn recordings(project: &Project) -> Vec<Media> {
    let mut all: Vec<Media> = Vec::new();
    for clip in &project.clips {
        let media = &clip.media;
        if media.kind == MediaKind::Recording && !all.iter().any(|m| m.path == media.path) {
            all.push(media.clone());
        }
    }
    all
}

fn load(dir: &ProjectDir, media: &Media) -> Result<Pressed, String> {
    let telemetry =
        Telemetry::read(&recording_dir(dir.root(), media)).map_err(|e| e.to_string())?;
    Ok(Pressed {
        clicks: telemetry.clicks(),
        keys: telemetry.key_presses(),
    })
}
