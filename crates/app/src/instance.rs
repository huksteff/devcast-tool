//! One app per user: the first one listens on the channel `devcast-app-<user>` (dc-ipc), a
//! second start asks it to show its window and quits. `devcast` asks on the same channel for the
//! project the editor holds (`dc_ops::live`, 7.32).

use std::sync::mpsc::{self, Receiver, Sender};

use dc_ipc::{IpcError, Listener};
use dc_ops::live::{Answer, Request, SERVICE};
use eframe::egui;

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
            dc_ipc::request::<Request, Answer>(name, &Request::Show)?;
            Ok(Claim::Running)
        }
        Err(error) => Err(error),
    }
}

/// A request of the channel for the main window; the answer goes back through `reply`.
pub struct Call {
    pub request: Request,
    pub reply: Sender<Answer>,
}

/// Takes requests on its own thread for the life of the process: each goes to the returned
/// receiver, wakes the main window and waits for its answer.
pub fn serve(listener: Listener, ctx: egui::Context) -> Receiver<Call> {
    let (calls, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let answered = listener.accept().and_then(|mut connection| {
                let request = connection.receive()?;
                let (reply, answer) = mpsc::channel();
                if calls.send(Call { request, reply }).is_err() {
                    return Ok(false);
                }
                ctx.request_repaint_of(egui::ViewportId::ROOT);
                match answer.recv() {
                    Ok(answer) => connection.send(&answer).map(|()| true),
                    Err(_) => Ok(false),
                }
            });
            match answered {
                Ok(true) => {}
                Ok(false) => return,
                Err(error) => tracing::warn!("запрос к DevCast не обработан: {error}"),
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
        let calls = serve(listener, egui::Context::default());
        let window = std::thread::spawn(move || {
            let call = calls.recv().unwrap();
            assert_eq!(call.request, Request::Show);
            call.reply.send(Answer::Shown).unwrap();
        });
        assert!(matches!(claim_on(&name).unwrap(), Claim::Running));
        window.join().unwrap();
    }
}
