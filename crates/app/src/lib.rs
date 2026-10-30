//! DevCast desktop app: HUD, recording windows, editor, timeline, library, settings.
//! `main.rs` only reports the error of [`run`].

mod app;
mod error;
mod instance;
mod logging;

pub use error::AppError;

use instance::Claim;

/// Name of the app for eframe, the window title and the inspection label.
const APP_NAME: &str = "DevCast";

/// Starts the app, or brings the running one to front and returns.
pub fn run() -> Result<(), AppError> {
    let settings = dc_project::settings::dir()?;
    let _log = logging::init(&settings.join(logging::FOLDER))?;
    let listener = match instance::claim()? {
        Claim::First(listener) => listener,
        Claim::Running => {
            tracing::info!("DevCast уже запущен: показано его окно");
            return Ok(());
        }
    };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default().with_title(APP_NAME),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            let shown = instance::serve(listener, cc.egui_ctx.clone());
            Ok(Box::new(app::App::new(shown)))
        }),
    )
    .map_err(|e| AppError::Window(e.to_string()))
}
