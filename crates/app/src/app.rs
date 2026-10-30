//! The eframe app: repaints only on events (decision 002), so it uses no CPU while idle.

use std::sync::mpsc::Receiver;

use eframe::egui;

pub struct App {
    /// A second start of the app asked to show this one.
    shown: Receiver<()>,
}

impl App {
    pub fn new(shown: Receiver<()>) -> Self {
        Self { shown }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shown.try_iter().count() > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(crate::APP_NAME);
        });
    }
}
