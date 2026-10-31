//! DevCast desktop app: HUD, recording windows, editor, timeline, library, settings.
//! `main.rs` only reports the error of [`run`].

mod app;
mod area;
mod countdown;
#[allow(
    unsafe_code,
    reason = "crash-handler's crash callback is an unsafe contract; the only unsafe of dc-app"
)]
mod crash;
#[cfg(test)]
mod e2e_tests;
mod editor;
mod error;
mod gpu;
mod hotkeys;
mod hud;
mod instance;
mod logging;
mod logo;
mod meters;
mod models;
mod recorder;
mod session;
mod settings;
mod settings_captions;
mod settings_hotkeys;
mod settings_rows;
mod settings_window;
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
        return crash::serve(std::path::Path::new(socket), dir.into());
    }
    let settings = dc_project::settings::dir()?;
    let _log = logging::init(&settings.join(logging::FOLDER))?;
    let _crash = crash::install(&settings)
        .inspect_err(|e| tracing::error!("дампы падений не будут записаны: {e}"));
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
        wgpu_options: gpu::options(),
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(logo::window_icon())
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
            without_frame(cc)?;
            let shown = instance::serve(listener, cc.egui_ctx.clone());
            let shared = app::Shared::new(cc.egui_ctx.clone(), store);
            let tray = tray::Tray::new(&cc.egui_ctx)?;
            let keys = hotkeys::GlobalKeys::new(&cc.egui_ctx)?;
            Ok(Box::new(app::App::new(shown, shared, hwnd, tray, keys)))
        }),
    )
    .map_err(|e| AppError::Window(e.to_string()))
}

/// egui asks winit for the shadow of a borderless window, and winit gets it by keeping one
/// row of the caption above the content: a light 1 px line across the top of the HUD on
/// Windows 10, and the content 1 px lower than the window. The HUD has its own rounded shape.
fn without_frame(cc: &eframe::CreationContext<'_>) -> Result<(), AppError> {
    use winit::platform::windows::WindowExtWindows;
    let window = cc
        .winit_window()
        .ok_or_else(|| AppError::Window("у главного окна нет окна winit".to_owned()))?;
    window.set_undecorated_shadow(false);
    Ok(())
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
