//! One app per user: the first one listens on the channel `devcast-app-<user>` (dc-ipc), a
//! second start asks it to show its window and quits.

use std::sync::mpsc::{self, Receiver};

use dc_ipc::{IpcError, Listener};
use eframe::egui;
use serde::{Deserialize, Serialize};

/// Service name of the channel (`dc_ipc::socket_name`).
const SERVICE: &str = "app";

/// What a second start asks of the running app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Request {
    Show,
}

pub enum Claim {
    /// This is the only app: it keeps the channel.
    First(Listener),
    /// Another app is running and was asked to show itself.
    Running,
}

pub fn claim() -> Result<Claim, IpcError> {
    claim_on(&dc_ipc::socket_name(SERVICE)?)
}

fn claim_on(name: &str) -> Result<Claim, IpcError> {
    match Listener::bind(name) {
        Ok(listener) => Ok(Claim::First(listener)),
        Err(IpcError::Taken { .. }) => {
            dc_ipc::request::<Request, ()>(name, &Request::Show)?;
            Ok(Claim::Running)
        }
        Err(error) => Err(error),
    }
}

/// Answers second starts on its own thread for the life of the process: each one sends `()`
/// to the returned receiver and wakes the UI.
pub fn serve(listener: Listener, ctx: egui::Context) -> Receiver<()> {
    let (shown, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let answered = listener.accept().and_then(|mut connection| {
                let Request::Show = connection.receive()?;
                connection.send(&())
            });
            match answered {
                Ok(()) if shown.send(()).is_ok() => ctx.request_repaint_of(egui::ViewportId::ROOT),
                Ok(()) => return,
                Err(error) => tracing::warn!("второй запуск DevCast не обработан: {error}"),
            }
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_start_shows_the_first() {
        let name = format!("devcast-test-app-{}", std::process::id());
        let Claim::First(listener) = claim_on(&name).unwrap() else {
            panic!("the first start must get the channel");
        };
        let shown = serve(listener, egui::Context::default());
        assert!(matches!(claim_on(&name).unwrap(), Claim::Running));
        assert_eq!(shown.recv(), Ok(()));
    }
}
