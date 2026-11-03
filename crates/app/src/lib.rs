//! DevCast desktop app: HUD, recording windows, editor, timeline, library, settings.
//! `main.rs` only reports the error of [`run`].

mod app;
mod area;
mod borderless;
mod countdown;
#[allow(
    unsafe_code,
    reason = "crash-handler's crash callback is an unsafe contract; the only unsafe of dc-app"
)]
mod crash;
mod downloads;
#[cfg(test)]
mod e2e_tests;
mod editor;
mod error;
mod exports;
mod gpu;
mod hotkeys;
mod hud;
mod instance;
mod keycaps;
mod keymap;
mod logging;
mod logo;
mod meters;
mod microphones;
mod models;
mod monitor;
mod recognition;
mod recorder;
mod remote;
mod session;
mod settings;
mod settings_captions;
mod settings_defaults;
mod settings_hotkeys;
mod settings_window;
mod shortcut;
mod tray;

pub use error::AppError;

use instance::Claim;

/// Name of the app for eframe, the window title and the inspection label.
const APP_NAME: &str = "DevCast";

/// Starts the app, or brings the running one to front and returns.
pub fn run() -> Result<(), AppError> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if let [_, flag, socket, dir] = args.as_slice()
        && flag == crash::SERVER_ARG
    {
        return crash::serve(std::path::Path::new(socket), dir.into(), std::io::stdin());
    }
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
    // Only the first instance writes dumps: a second start must not leave a dump writer behind.
    let _crash = crash::install(&settings)
        .inspect_err(|e| tracing::error!("дампы падений не будут записаны: {e}"));
    let gpu = gpu::device()?;
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: gpu::options(&gpu),
        viewport: borderless::floating(APP_NAME)
            .with_inner_size(borderless::outer_size(hud::START_SIZE))
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            dc_ui_kit::install(&cc.egui_ctx)?;
            let hwnd = window_handle(cc)?;
            let calls = instance::serve(listener, cc.egui_ctx.clone());
            let mut shared = app::Shared::new(cc.egui_ctx.clone(), store);
            let state = cc
                .wgpu_render_state
                .as_ref()
                .ok_or_else(|| AppError::Window("окна рисуются без wgpu".to_owned()))?;
            shared.gpu = Some(gpu::PreviewGpu::new(gpu, state));
            // The real screens and windows: `Shared::new` lists none, so tests never ask the system.
            shared.refresh_sources();
            // Ready before the sound popover first opens: it then opens at its height.
            shared.mic_refresh.start(&cc.egui_ctx);
            let tray = tray::Tray::new(&cc.egui_ctx)?;
            let keys = hotkeys::GlobalKeys::new(&cc.egui_ctx)?;
            Ok(Box::new(app::App::new(calls, shared, hwnd, tray, keys)))
        }),
    )
    .map_err(|e| AppError::Window(e.to_string()))
}

/// Opens `path` in Explorer.
pub(crate) fn open_folder(path: &std::path::Path) -> Result<(), AppError> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(drop)
        .map_err(|source| AppError::OpenFolder {
            path: path.to_path_buf(),
            source,
        })
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
