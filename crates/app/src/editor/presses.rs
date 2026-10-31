//! The clicks of the recordings (7.15): the button presses of each recording's telemetry
//! (`dc_capture::input::Telemetry::clicks`, the same as `devcast events clicks`), read on the
//! thread `devcast-clicks` (`reader.rs`). The «Курсор» row shows their dots once they come.

use dc_capture::input::{Click, Telemetry};
use dc_capture::project::recording_dir;
use dc_core::Project;
use dc_core::project::{Media, MediaKind};
use dc_project::ProjectDir;
use eframe::egui;

use super::reader::{Reader, Wanted};

/// The presses of each recording, in time order.
pub type Presses = Reader<Media, Vec<Click>>;

impl Wanted for Media {
    const THREAD: &'static str = "devcast-clicks";
    const FAILED: &'static str = "клики записи не прочитаны";
}

impl Presses {
    /// Starts the thread that reads the clicks of the project in `dir`; it wakes the main window
    /// of `ctx` after every answer.
    pub fn start(&mut self, dir: ProjectDir, ctx: egui::Context) -> std::io::Result<()> {
        let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.start_with(move |media| load(&dir, media), wake)
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

fn load(dir: &ProjectDir, media: &Media) -> Result<Vec<Click>, String> {
    let telemetry =
        Telemetry::read(&recording_dir(dir.root(), media)).map_err(|e| e.to_string())?;
    Ok(telemetry.clicks())
}
