//! Spike 1.1: transparent, always-on-top, click-through HUD on eframe + wgpu
//! (DX12, DxgiFromVisual swapchain), excluded from screen capture.
//!
//! Flags: --exclude (WDA_EXCLUDEFROMCAPTURE), --passthrough (MousePassthrough after first frames),
//! --hwnd-swapchain (DxgiFromHwnd, to compare), --seconds N (auto close).

use std::time::{Duration, Instant};

use eframe::{egui, egui_wgpu, wgpu};

#[derive(Clone, Copy, Default)]
struct Args {
    exclude: bool,
    passthrough: bool,
    hwnd_swapchain: bool,
    no_redirect: bool,
    opaque: bool,
    seconds: Option<f64>,
    pos: Option<[f32; 2]>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--exclude" => args.exclude = true,
            "--passthrough" => args.passthrough = true,
            "--hwnd-swapchain" => args.hwnd_swapchain = true,
            "--no-redirect" => args.no_redirect = true,
            "--opaque" => args.opaque = true,
            "--seconds" => args.seconds = it.next().and_then(|s| s.parse().ok()),
            "--pos" => {
                let x = it.next().and_then(|s| s.parse().ok());
                let y = it.next().and_then(|s| s.parse().ok());
                args.pos = x.zip(y).map(|(x, y)| [x, y]);
            }
            other => eprintln!("unknown arg {other}"),
        }
    }
    args
}

fn main() -> eframe::Result {
    env_logger::init();
    let args = parse_args();
    let mut wgpu_options = egui_wgpu::WgpuConfiguration::default();
    if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = wgpu::Backends::DX12;
        setup.instance_descriptor.backend_options.dx12.presentation_system = if args.hwnd_swapchain {
            wgpu::Dx12SwapchainKind::DxgiFromHwnd
        } else {
            wgpu::Dx12SwapchainKind::DxgiFromVisual
        };
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        viewport: egui::ViewportBuilder::default()
            .with_title("spike-hud")
            .with_inner_size(if args.opaque { [268.0, 52.0] } else { [420.0, 140.0] })
            .with_position(args.pos.unwrap_or([200.0, 200.0]))
            .with_transparent(!args.opaque)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false),
        ..Default::default()
    };
    eframe::run_native(
        "spike-hud",
        options,
        Box::new(move |cc| {
            if let Some(rs) = cc.wgpu_render_state.as_ref() {
                let info = rs.adapter.get_info();
                println!("adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
            }
            if args.opaque {
                round_corners(cc)?;
                println!("dwm: rounded corners");
            }
            if args.no_redirect {
                set_no_redirection_bitmap(cc)?;
                println!("ex-style: WS_EX_NOREDIRECTIONBITMAP added after creation");
            }
            if args.exclude {
                exclude_from_capture(cc)?;
                println!("display affinity: WDA_EXCLUDEFROMCAPTURE set");
            }
            Ok(Box::new(Hud { started: Instant::now(), args, passthrough_sent: false }))
        }),
    )
}

fn exclude_from_capture(
    handle: &impl raw_window_handle::HasWindowHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
    let raw_window_handle::RawWindowHandle::Win32(win32) = handle.window_handle()?.as_raw() else {
        return Err("not a Win32 window".into());
    };
    let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
    // SAFETY: hwnd is the live top-level window owned by this process (from winit).
    unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)? };
    Ok(())
}

struct Hud {
    started: Instant,
    args: Args,
    passthrough_sent: bool,
}

impl eframe::App for Hud {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.args.opaque { [24.0 / 255.0, 24.0 / 255.0, 28.0 / 255.0, 1.0] } else { [0.0; 4] }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let elapsed = self.started.elapsed().as_secs_f64();
        if self.args.passthrough && !self.passthrough_sent && elapsed > 0.5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(true));
            self.passthrough_sent = true;
            println!("mouse passthrough: on");
        }
        if self.args.seconds.is_some_and(|limit| elapsed > limit) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(24, 24, 28, 235))
            .corner_radius(14)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_width(240.0);
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(235, 64, 52), "● REC");
                    let secs = self.started.elapsed().as_secs();
                    ui.label(format!("{:02}:{:02}", secs / 60, secs % 60));
                    let _ = ui.button("Стоп");
                });
            });
    }
}

fn hwnd_of(
    handle: &impl raw_window_handle::HasWindowHandle,
) -> Result<windows::Win32::Foundation::HWND, Box<dyn std::error::Error + Send + Sync>> {
    let raw_window_handle::RawWindowHandle::Win32(win32) = handle.window_handle()?.as_raw() else {
        return Err("not a Win32 window".into());
    };
    Ok(windows::Win32::Foundation::HWND(win32.hwnd.get() as *mut core::ffi::c_void))
}

fn set_no_redirection_bitmap(
    handle: &impl raw_window_handle::HasWindowHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SetWindowLongPtrW, SetWindowPos, WS_EX_NOREDIRECTIONBITMAP,
    };
    let hwnd = hwnd_of(handle)?;
    // SAFETY: hwnd is the live top-level window owned by this process (from winit).
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOREDIRECTIONBITMAP.0 as isize);
        SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED)?;
    }
    Ok(())
}

fn round_corners(
    handle: &impl raw_window_handle::HasWindowHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    let hwnd = hwnd_of(handle)?;
    let preference = DWMWCP_ROUND;
    // SAFETY: hwnd is our live window; the pointer and size describe `preference`.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const preference).cast(),
            size_of_val(&preference) as u32,
        )?;
    }
    Ok(())
}
