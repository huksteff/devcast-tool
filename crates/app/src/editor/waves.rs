//! Peaks of the waves on the sound rows (7.11), read on the thread `devcast-peaks` (`reader.rs`):
//! a sound row is drawn plain until its peaks come. The peaks of a track are built once and kept
//! in the project cache by `dc_audio::cached_peaks`.

use dc_audio::peaks::{Peaks, cached_peaks};
use dc_audio::{file_track, tracks};
use dc_project::ProjectDir;
use eframe::egui;

use super::reader::{Reader, Wanted};
use super::timeline::Source;

pub type Waves = Reader<Source, Peaks>;

impl Wanted for Source {
    const THREAD: &'static str = "devcast-peaks";
    const FAILED: &'static str = "волна звука не прочитана";
}

impl Waves {
    /// Starts the thread that reads the peaks of the project in `dir`; it wakes the main window
    /// of `ctx` after every answer.
    pub fn start(&mut self, dir: ProjectDir, ctx: egui::Context) -> std::io::Result<()> {
        let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.start_with(move |source| load(&dir, source), wake)
    }
}

/// The peaks of `source` in the project `dir`, from its cache when it has them.
pub(super) fn load(dir: &ProjectDir, source: &Source) -> Result<Peaks, String> {
    let track = match source {
        Source::Track { media, name } => tracks(dir, media)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|t| t.name == *name)
            .ok_or_else(|| format!("у {} нет дорожки {name}", media.path))?,
        Source::File(path) => {
            let file = path
                .split('/')
                .fold(dir.root().to_path_buf(), |p, part| p.join(part));
            file_track(&file).map_err(|e| e.to_string())?
        }
    };
    cached_peaks(dir, &track).map_err(|e| e.to_string())
}
