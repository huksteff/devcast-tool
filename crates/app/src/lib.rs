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
mod crashes;
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
mod settings_about;
mod settings_captions;
mod settings_defaults;
mod settings_hotkeys;
mod settings_window;
mod shortcut;
mod tray;

pub use error::AppError;

use instance::Claim;

/// Name of the app for eframe and the window title.
const APP_NAME: &str = "DevCast";

/// Starts the app, or brings the running one to front and returns.
pub fn run() -> Result<(), AppError> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if let [_, flag, socket, dir] = args.as_slice()
        && flag == crash::SERVER_ARG
    {
        return crash::serve(std::path::Path::new(socket), dir.into(), std::io::stdin());
    }
    let started = std::time::Instant::now();
    let settings = dc_project::settings::dir()?;
    let logs = settings.join(dc_project::settings::LOGS_DIR);
    let crashes = logs.join(dc_project::settings::CRASHES_DIR);
    let _log = logging::init(&logs, &crashes)?;
    let store = settings::Store::load(&settings.join(settings::FILE))?;
    let listener = match instance::claim()? {
        Claim::First(listener) => listener,
        Claim::Running => {
            tracing::info!("DevCast уже запущен: показано его окно");
            return Ok(());
        }
    };
    // Only the first instance writes dumps: a second start must not leave a dump writer behind.
    let _crash = crash::install(&crashes)
        .inspect_err(|e| tracing::error!("дампы падений не будут записаны: {e}"));
    tracing::debug!(ms = started.elapsed().as_millis(), "старт: дампы падений");
    // Listing windows asks WinRT about each (~0.35 s): it runs while the GPU starts.
    let sources = std::thread::Builder::new()
        .name("devcast-sources".to_owned())
        .spawn(dc_capture::win::list_sources);
    let gpu = gpu::device()?;
    tracing::debug!(ms = started.elapsed().as_millis(), "старт: GPU");
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
            match sources.ok().and_then(|thread| thread.join().ok()) {
                Some(listed) => shared.take_sources(listed),
                None => shared.refresh_sources(),
            }
            // Ready before the sound popover first opens: it then opens at its height.
            shared.mic_refresh.start(&cc.egui_ctx);
            let told = hud::notices::at_start(&mut shared, &crashes, env!("CARGO_PKG_VERSION"));
            shared.report(told);
            let tray = tray::Tray::new(&cc.egui_ctx)?;
            let keys = hotkeys::GlobalKeys::new(&cc.egui_ctx)?;
            tracing::debug!(ms = started.elapsed().as_millis(), "старт: готово");
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

/// Opens the folder of the logs (`%APPDATA%\DevCast\logs`) in Explorer.
pub(crate) fn open_logs() -> Result<(), AppError> {
    open_folder(&dc_project::settings::logs_dir()?)
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
