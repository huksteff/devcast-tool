//! The sound tracks of the imported videos (`dc_audio::tracks`: `audio-1`, `audio-2` … in file
//! order), read on the thread `devcast-streams` (`reader.rs`) for the sound panel of a clip.

use dc_audio::tracks;
use dc_core::Project;
use dc_core::project::{Media, MediaKind};
use dc_project::ProjectDir;
use eframe::egui;

use super::reader::{Reader, Wanted};

/// An imported video of the project.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported(pub Media);

impl Wanted for Imported {
    const THREAD: &'static str = "devcast-streams";
    const FAILED: &'static str = "дорожки звука видео не прочитаны";
}

/// The names of the sound tracks of each imported video.
pub type Streams = Reader<Imported, Vec<String>>;

impl Streams {
    /// Starts the thread that reads the tracks of the project in `dir`; it wakes the main window
    /// of `ctx` after every answer.
    pub fn start(&mut self, dir: ProjectDir, ctx: egui::Context) -> std::io::Result<()> {
        let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.start_with(move |imported| load(&dir, imported), wake)
    }
}

/// The imported videos of the clips of `project`, once each.
pub fn imported(project: &Project) -> Vec<Imported> {
    let mut all: Vec<Imported> = Vec::new();
    for clip in &project.clips {
        let media = &clip.media;
        if media.kind == MediaKind::Imported && !all.iter().any(|m| m.0.path == media.path) {
            all.push(Imported(media.clone()));
        }
    }
    all
}

fn load(dir: &ProjectDir, Imported(media): &Imported) -> Result<Vec<String>, String> {
    let found = tracks(dir, media).map_err(|e| e.to_string())?;
    Ok(found.into_iter().map(|t| t.name).collect())
}
