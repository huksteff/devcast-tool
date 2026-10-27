//! Spike 1.11: CLI → running editor over a local socket (named pipe on Windows).
//! `server`: egui window on the LEFT monitor; each command changes a value; after the frame
//! showing the new value is painted, the server acknowledges. `client <n>`: sends n commands and
//! measures send → "painted" latency. Without a server the client reports it cleanly.
//!
//! Usage: spike-ipc server [seconds] | spike-ipc client <n>

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream, prelude::*};
use serde::{Deserialize, Serialize};

const NAME: &str = "devcast-spike-ipc.sock";

#[derive(Serialize, Deserialize)]
struct Command {
    id: u32,
    set_value: i64,
}

#[derive(Serialize, Deserialize)]
struct Ack {
    id: u32,
    painted_value: i64,
}

/// A command plus the way to answer the client that sent it.
struct Pending {
    command: Command,
    reply: Sender<Ack>,
}

fn serve(ctx: egui::Context, tx: Sender<Pending>) -> std::io::Result<()> {
    let name = NAME.to_ns_name::<GenericNamespaced>()?;
    let listener = ListenerOptions::new().name(name).create_sync()?;
    for conn in listener.incoming().filter_map(Result::ok) {
        let tx = tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(conn);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                let Ok(command) = serde_json::from_str::<Command>(&line) else { break };
                line.clear();
                let (reply_tx, reply_rx) = channel();
                if tx.send(Pending { command, reply: reply_tx }).is_err() {
                    break;
                }
                ctx.request_repaint();
                let Ok(ack) = reply_rx.recv() else { break };
                let Ok(mut json) = serde_json::to_string(&ack) else { break };
                json.push('\n');
                if reader.get_mut().write_all(json.as_bytes()).is_err() {
                    break;
                }
            }
        });
    }
    Ok(())
}

struct Editor {
    rx: Receiver<Pending>,
    value: i64,
    to_ack: Vec<(Ack, Sender<Ack>)>,
    started: Instant,
    seconds: f64,
}

impl eframe::App for Editor {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(p) = self.rx.try_recv() {
            self.value = p.command.set_value;
            self.to_ack.push((Ack { id: p.command.id, painted_value: self.value }, p.reply));
        }
        if self.started.elapsed().as_secs_f64() > self.seconds {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(format!("Значение из CLI: {}", self.value));
        });
        // The frame with the new value has been laid out; acknowledge the commands it shows.
        for (ack, reply) in self.to_ack.drain(..) {
            let _ = reply.send(ack);
        }
    }
}

fn client(n: u32) -> Result<(), Box<dyn std::error::Error>> {
    let name = NAME.to_ns_name::<GenericNamespaced>()?;
    let started = Instant::now();
    let conn = match Stream::connect(name) {
        Ok(c) => c,
        Err(e) => {
            println!("editor is not running ({:?} after {:.1} ms): fall back to editing files directly", e.kind(), started.elapsed().as_secs_f64() * 1000.0);
            return Ok(());
        }
    };
    let mut reader = BufReader::new(conn);
    let mut times = Vec::new();
    for id in 0..n {
        let sent = Instant::now();
        let mut json = serde_json::to_string(&Command { id, set_value: i64::from(id) * 10 })?;
        json.push('\n');
        reader.get_mut().write_all(json.as_bytes())?;
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let ack: Ack = serde_json::from_str(&line)?;
        assert_eq!(ack.painted_value, i64::from(id) * 10);
        times.push(sent.elapsed().as_secs_f64() * 1000.0);
        std::thread::sleep(Duration::from_millis(30));
    }
    times.sort_by(f64::total_cmp);
    println!(
        "commands={n} latency send→painted: p50={:.1} ms p95={:.1} ms max={:.1} ms",
        times[times.len() / 2],
        times[times.len() * 95 / 100],
        times[times.len() - 1]
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("client") => client(args.get(1).and_then(|s| s.parse().ok()).unwrap_or(50)),
        Some("server") => {
            let seconds = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10.0);
            let options = eframe::NativeOptions {
                renderer: eframe::Renderer::Wgpu,
                viewport: egui::ViewportBuilder::default()
                    .with_title("spike-ipc")
                    .with_inner_size([420.0, 120.0])
                    .with_position([-1700.0, 300.0])
                    .with_always_on_top(),
                ..Default::default()
            };
            eframe::run_native(
                "spike-ipc",
                options,
                Box::new(move |cc| {
                    let (tx, rx) = channel();
                    let ctx = cc.egui_ctx.clone();
                    std::thread::spawn(move || serve(ctx, tx));
                    Ok(Box::new(Editor { rx, value: 0, to_ack: Vec::new(), started: Instant::now(), seconds }))
                }),
            )?;
            Ok(())
        }
        _ => Err("usage: server [seconds] | client <n>".into()),
    }
}
