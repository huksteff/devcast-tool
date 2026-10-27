//! Spike 1.2: editor + detached timeline + HUD as three native windows (eframe deferred viewports).
//! Checks that secondary windows keep repainting while the root has focus (egui #8466),
//! reports per-window DPI, and allows measuring idle CPU.
//!
//! Flags: --mode anim|idle, --seconds N. Prints frames per window at exit.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use eframe::egui;

const FRAME: Duration = Duration::from_millis(33);

#[derive(Default)]
struct Counters {
    root: AtomicU64,
    timeline: AtomicU64,
    hud: AtomicU64,
    ppp: std::sync::Mutex<Vec<(&'static str, f32)>>,
}

fn main() -> eframe::Result {
    let mut animate = true;
    let mut seconds = 6.0;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => animate = args.next().as_deref() != Some("idle"),
            "--seconds" => seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(seconds),
            other => eprintln!("unknown arg {other}"),
        }
    }
    let counters = Arc::new(Counters::default());
    let report = Arc::clone(&counters);
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("spike-editor")
            .with_inner_size([860.0, 420.0])
            .with_position([-1880.0, 160.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "spike-windows",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(Editor { started: Instant::now(), animate, seconds, counters }))
        }),
    );
    let ppp = report.ppp.lock().map(|v| v.clone()).unwrap_or_default();
    println!(
        "frames: root={} timeline={} hud={} over {seconds}s (animate={animate}); pixels_per_point: {ppp:?}",
        report.root.load(Ordering::Relaxed),
        report.timeline.load(Ordering::Relaxed),
        report.hud.load(Ordering::Relaxed),
    );
    result
}

struct Editor {
    started: Instant,
    animate: bool,
    seconds: f64,
    counters: Arc<Counters>,
}

fn record_ppp(counters: &Counters, name: &'static str, ppp: f32) {
    if let Ok(mut list) = counters.ppp.lock()
        && !list.iter().any(|(n, _)| *n == name)
    {
        list.push((name, ppp));
    }
}

impl eframe::App for Editor {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.started.elapsed().as_secs_f64() > self.seconds {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let animate = self.animate;
        let started = self.started;
        let c = Arc::clone(&self.counters);
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of("timeline"),
            egui::ViewportBuilder::default()
                .with_title("spike-timeline")
                .with_inner_size([860.0, 260.0])
                .with_position([-1880.0, 640.0]),
            move |ui, _class| {
                c.timeline.fetch_add(1, Ordering::Relaxed);
                record_ppp(&c, "timeline", ui.ctx().pixels_per_point());
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.heading("Таймлайн (отдельное окно)");
                    ui.label(format!("t = {:.2} c", started.elapsed().as_secs_f64()));
                });
                if animate {
                    ui.ctx().request_repaint_after(FRAME);
                }
            },
        );
        let c = Arc::clone(&self.counters);
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of("hud"),
            egui::ViewportBuilder::default()
                .with_title("spike-hud2")
                .with_inner_size([260.0, 52.0])
                .with_position([-960.0, 160.0])
                .with_decorations(false)
                .with_always_on_top(),
            move |ui, _class| {
                c.hud.fetch_add(1, Ordering::Relaxed);
                record_ppp(&c, "hud", ui.ctx().pixels_per_point());
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.label(format!("REC {:.1} c", started.elapsed().as_secs_f64()));
                });
                if animate {
                    ui.ctx().request_repaint_after(FRAME);
                }
            },
        );
        if animate {
            ctx.request_repaint_after(FRAME);
        } else {
            // Wake once at the deadline so the idle run can close itself.
            let left = self.seconds - self.started.elapsed().as_secs_f64();
            ctx.request_repaint_after(Duration::from_secs_f64(left.max(0.0) + 0.05));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.counters.root.fetch_add(1, Ordering::Relaxed);
        record_ppp(&self.counters, "editor", ui.ctx().pixels_per_point());
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Редактор (главное окно, в фокусе)");
            ui.label(format!("t = {:.2} c", self.started.elapsed().as_secs_f64()));
        });
    }
}
