//! The preview's link to its render thread: asks for frames, takes the answers and shows the
//! texture the thread drew into as an egui texture of the same device (no copy through the
//! CPU). The thread wakes the main window only (app CLAUDE.md); the main window's pass wakes the
//! editor when there is news (`take_news`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dc_core::Project;
use dc_project::ProjectDir;
use dc_ui_kit::strings::editor as text;
use eframe::egui::{self, TextureId};
use eframe::wgpu;

use super::fetch::{Fetch, Want};
use super::worker::{Rendered, Request, Worker};
use crate::gpu::PreviewGpu;

#[derive(Debug)]
pub struct Live {
    gpu: PreviewGpu,
    worker: Worker,
    fetch: Fetch<Rendered>,
    /// The egui texture of the shown frame and the serial of the texture it shows.
    texture: Option<(TextureId, u64)>,
    news: Arc<AtomicBool>,
}

/// What the preview paints: the frame, or why there is none.
#[derive(Debug, Default)]
pub struct Shown {
    pub texture: Option<TextureId>,
    pub error: Option<String>,
    /// The camera of the shown frame (px of the output frame); none without a frame.
    pub camera: Option<dc_render::Camera>,
}

impl Live {
    pub fn start(gpu: PreviewGpu, dir: ProjectDir, ctx: egui::Context) -> std::io::Result<Self> {
        let news = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&news);
        let worker = Worker::start(Arc::clone(&gpu.gpu), dir, move || {
            flag.store(true, Ordering::Release);
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        })?;
        Ok(Self {
            gpu,
            worker,
            fetch: Fetch::default(),
            texture: None,
            news,
        })
    }

    /// The thread answered since the last call.
    pub fn take_news(&self) -> bool {
        self.news.swap(false, Ordering::AcqRel)
    }

    /// Asks for `want` of `project` if it is new and shows the newest answer.
    pub fn frame(&mut self, project: &Arc<Project>, want: Want) -> Shown {
        if let Some(generation) = self.fetch.want(want) {
            let request = Request {
                generation,
                project: Arc::clone(project),
                want,
            };
            if !self.worker.send(request) {
                self.fetch.lost(text::PREVIEW_STOPPED.to_owned());
            }
        }
        for (generation, answer) in self.worker.answers() {
            self.fetch.answer(generation, answer);
        }
        let Some(rendered) = self.fetch.shown() else {
            return Shown {
                texture: None,
                error: self.fetch.error().map(str::to_owned),
                camera: None,
            };
        };
        let serial = rendered.serial;
        let camera = rendered.camera;
        let id = match self.texture {
            Some((id, shown)) if shown == serial => id,
            known => {
                let view = rendered
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut renderer = self.gpu.renderer.write();
                let device = &self.gpu.gpu.device;
                let filter = wgpu::FilterMode::Linear;
                match known {
                    Some((id, _)) => {
                        renderer.update_egui_texture_from_wgpu_texture(device, &view, filter, id);
                        id
                    }
                    None => renderer.register_native_texture(device, &view, filter),
                }
            }
        };
        self.texture = Some((id, serial));
        Shown {
            texture: Some(id),
            error: None,
            camera: Some(camera),
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some((id, _)) = self.texture {
            self.gpu.renderer.write().free_texture(&id);
        }
    }
}
