//! Pictures of the library tiles, read on the thread `devcast-thumbs` (`reader.rs`): the preview
//! of the project's first clip (`dc_media::project_preview`, built once and kept in that
//! project's `cache/`, like the peaks of the waves). A tile is plain until its picture comes.

use std::path::PathBuf;

use dc_project::ProjectDir;
use eframe::egui::{self, ColorImage};

use crate::editor::reader::{Reader, Wanted};

/// The project folder of a recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Thumb(pub PathBuf);

impl Wanted for Thumb {
    const THREAD: &'static str = "devcast-thumbs";
    const FAILED: &'static str = "миниатюра записи не построена";
}

pub type Thumbs = Reader<Thumb, ColorImage>;

impl Thumbs {
    /// Starts the thread; it wakes the main window of `ctx` after every picture.
    pub fn start(&mut self, ctx: egui::Context) -> std::io::Result<()> {
        let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.start_with(load, wake)
    }
}

fn load(Thumb(root): &Thumb) -> Result<ColorImage, String> {
    let dir = ProjectDir::open(root).map_err(|e| e.to_string())?;
    let project = dir.load().map_err(|e| e.to_string())?;
    let file = dc_media::project_preview(&dir, &project)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("в {} нет клипов", root.display()))?;
    let picture = dc_media::decode_picture(&file).map_err(|e| e.to_string())?;
    let frame = picture.frames.first().ok_or("пустая картинка")?;
    let size = [picture.size.width as usize, picture.size.height as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, &frame.rgba))
}
