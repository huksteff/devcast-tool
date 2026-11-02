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
    pub reply: Reply,
}

/// The way back to the run that asked. The main window may keep it until the answer is known
/// (`record stop` answers once the project is saved): each request waits on its own thread.
pub struct Reply {
    answer: Sender<Answer>,
    /// Closes once the answer is written to the run (or could not be).
    written: Receiver<()>,
}

impl Reply {
    pub fn send(self, answer: Answer) {
        // The run that asked may have given up waiting; nothing to answer then.
        let _ = self.answer.send(answer);
    }

    /// Sends and waits until the answer is written: the app ends right after it.
    pub fn send_written(self, answer: Answer) {
        if self.answer.send(answer).is_ok() {
            let _ = self.written.recv();
        }
    }

    /// A reply whose answer the test reads (no channel).
    #[cfg(test)]
    pub fn fake() -> (Self, Receiver<Answer>) {
        let (answer, answers) = mpsc::channel();
        let (_written, written) = mpsc::channel();
        (Self { answer, written }, answers)
    }
}

/// Takes requests on its own thread for the life of the process: each goes to the returned
/// receiver and wakes the main window; its answer is awaited and written on a thread of its own,
/// so a request whose answer takes long (a stop, an export) does not hold up the others.
pub fn serve(listener: Listener, ctx: egui::Context) -> Receiver<Call> {
    let (calls, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let mut connection = match listener.accept() {
                Ok(connection) => connection,
                Err(error) => {
                    tracing::warn!("запрос к DevCast не принят: {error}");
                    continue;
                }
            };
            let request = match connection.receive() {
                Ok(request) => request,
                Err(error) => {
                    tracing::warn!("запрос к DevCast не прочитан: {error}");
                    continue;
                }
            };
            let (answer, answered) = mpsc::channel();
            let (written, done) = mpsc::channel::<()>();
            let reply = Reply {
                answer,
                written: done,
            };
            if calls.send(Call { request, reply }).is_err() {
                return;
            }
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            std::thread::spawn(move || {
                // No answer: the app has ended without one; the run sees the channel close.
                if let Ok(answer) = answered.recv()
                    && let Err(error) = connection.send(&answer)
                {
                    tracing::warn!("ответ DevCast не отправлен: {error}");
                }
                drop(written);
            });
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
            call.reply.send(Answer::Shown);
        });
        assert!(matches!(claim_on(&name).unwrap(), Claim::Running));
        window.join().unwrap();
    }
}
