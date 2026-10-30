//! DevCast desktop app: HUD, recording windows, editor, timeline, library, settings.
//! `main.rs` only reports the error of [`run`].

mod app;
mod error;
mod hud;
mod instance;
mod logging;
mod settings;
mod settings_window;

pub use error::AppError;

use instance::Claim;

/// Name of the app for eframe, the window title and the inspection label.
const APP_NAME: &str = "DevCast";

/// Starts the app, or brings the running one to front and returns.
pub fn run() -> Result<(), AppError> {
    let settings = dc_project::settings::dir()?;
    let _log = logging::init(&settings.join(logging::FOLDER))?;
    let store = settings::Store::load(&settings.join(settings::FILE))?;
    let listener = match instance::claim()? {
        Claim::First(listener) => listener,
        Claim::Running => {
            tracing::info!("DevCast уже запущен: показано его окно");
            return Ok(());
        }
    };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size(hud::START_SIZE)
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            dc_ui_kit::install(&cc.egui_ctx)?;
            let hwnd = window_handle(cc)?;
            let shown = instance::serve(listener, cc.egui_ctx.clone());
            let shared = app::Shared::new(cc.egui_ctx.clone(), store);
            Ok(Box::new(app::App::new(shown, shared, hwnd)))
        }),
    )
    .map_err(|e| AppError::Window(e.to_string()))
}

/// The Win32 handle of the main window (the HUD).
fn window_handle(cc: &eframe::CreationContext<'_>) -> Result<isize, AppError> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = cc
        .window_handle()
        .map_err(|e| AppError::Window(e.to_string()))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Ok(win32.hwnd.get()),
        other => Err(AppError::Window(format!("окно не Win32: {other:?}"))),
    }
}
