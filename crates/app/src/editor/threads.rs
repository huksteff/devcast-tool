//! Starting the threads of the editor window: the preview, the sound of playback, the waves,
//! the presses and the tracks of imported videos. Each wakes the main
//! window (app CLAUDE.md); without its thread the window goes on with less.

use eframe::egui;

use super::EditorWindow;

impl EditorWindow {
    /// Starts rendering the preview on the shared device; the thread wakes the main window.
    pub fn start_preview(
        &mut self,
        gpu: crate::gpu::PreviewGpu,
        ctx: egui::Context,
    ) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.preview
            .start(gpu, dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-preview",
                source,
            })
    }

    /// Starts the sound thread of playback; without it playback goes by the wall clock.
    pub fn start_sound(&mut self) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.playback
            .start(dir)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-sound",
                source,
            })
    }

    /// Starts reading the peaks of the sound rows; without it the rows stay plain.
    pub fn start_waves(&mut self, ctx: egui::Context) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.waves
            .start(dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-peaks",
                source,
            })
    }

    /// Starts reading the presses of the recordings; without it the «Курсор» row has no dots
    /// and the «Клавиши» row no shortcuts.
    pub fn start_presses(&mut self, ctx: egui::Context) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.presses
            .start(dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-presses",
                source,
            })
    }

    /// Starts reading the sound tracks of the imported videos; without it their clips' sound
    /// panel shows no tracks.
    pub fn start_streams(&mut self, ctx: egui::Context) -> Result<(), crate::AppError> {
        let dir = self.doc.dir().clone();
        self.streams
            .start(dir, ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-streams",
                source,
            })
    }
}
