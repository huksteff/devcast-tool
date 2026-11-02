//! Listing the microphones off the UI thread for the HUD's sound popover: Windows takes some
//! 100 ms to list them, and the popover opens at once with the list known, then takes the fresh
//! one. The thread wakes only the main window (decision 002); it passes the repaint on.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui;

/// The listing under way, if any.
#[derive(Debug, Default)]
pub struct Refresh(Option<Receiver<Result<Vec<String>, String>>>);

impl Refresh {
    /// Starts listing anew; a listing still running is dropped.
    pub fn start(&mut self, ctx: &egui::Context) {
        let (done, listed) = channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("devcast-mics".to_owned())
            .spawn(move || {
                let names = dc_capture::audio::list_microphones()
                    .map(|list| list.into_iter().map(|m| m.name).collect())
                    .map_err(|e| e.to_string());
                // The popover is closed when the send fails: nobody needs the list then.
                let _ = done.send(names);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            });
        match spawned {
            Ok(_) => self.0 = Some(listed),
            Err(error) => tracing::warn!("микрофоны не перечислены: {error}"),
        }
    }

    /// The fresh list once it came, taken once.
    pub fn take(&mut self) -> Option<Result<Vec<String>, String>> {
        let listed = self.0.as_ref()?.try_recv();
        match listed {
            Ok(names) => {
                self.0 = None;
                Some(names)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.0 = None;
                None
            }
        }
    }
}
