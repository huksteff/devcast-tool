//! One app per user: the first one listens on the channel `devcast-app-<user>` (dc-ipc), a
//! second start asks it to show its window and quits. `devcast` asks on the same channel for the
//! project the editor holds (`dc_ops::live`, 7.32).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

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

/// How long a request waits for the main window to take it. The window takes requests every
/// frame; it does not while a system dialog («Сохранить как») blocks it, and `devcast` — the
/// user's AI agent with it — must not hang until the dialog closes.
const PICKUP: Duration = Duration::from_secs(5);
const BUSY: &str =
    "DevCast занят и не ответил за 5 с: закрой открытое в нём окно выбора файла и повтори команду";

/// Whose a request is: the main window's once it takes it, nobody's once its run gave up.
const WAITING: u8 = 0;
const TAKEN: u8 = 1;
const GIVEN_UP: u8 = 2;

/// The requests of the channel, for the main window.
pub struct Calls(Receiver<(Call, Arc<AtomicU8>)>);

impl Calls {
    /// The requests that came and still wait for their answer: one whose run gave up waiting
    /// is not done later behind its back.
    pub fn try_iter(&self) -> impl Iterator<Item = Call> + '_ {
        self.0.try_iter().filter_map(taken)
    }

    /// Waits for the next request (tests).
    #[cfg(test)]
    pub fn recv(&self) -> Option<Call> {
        std::iter::from_fn(|| self.0.recv().ok()).find_map(taken)
    }
}

fn taken((call, state): (Call, Arc<AtomicU8>)) -> Option<Call> {
    let mine = state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst);
    mine.is_ok().then_some(call)
}

/// Takes requests on its own thread for the life of the process: each goes to the returned
/// [`Calls`] and wakes the main window; its answer is awaited and written on a thread of its own,
/// so a request whose answer takes long (a stop, an export) does not hold up the others.
pub fn serve(listener: Listener, ctx: egui::Context) -> Calls {
    serve_within(listener, ctx, PICKUP)
}

/// [`serve`] where the main window has `pickup` to take a request.
fn serve_within(listener: Listener, ctx: egui::Context, pickup: Duration) -> Calls {
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
            let state = Arc::new(AtomicU8::new(WAITING));
            if calls
                .send((Call { request, reply }, Arc::clone(&state)))
                .is_err()
            {
                return;
            }
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            std::thread::spawn(move || {
                let answer = match answered.recv_timeout(pickup) {
                    Ok(answer) => Some(answer),
                    // Not taken in time: the run hears why. Taken: the answer may take long.
                    Err(RecvTimeoutError::Timeout) => {
                        let given_up = state.compare_exchange(
                            WAITING,
                            GIVEN_UP,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        );
                        match given_up {
                            Ok(_) => Some(Answer::Refused(BUSY.to_owned())),
                            Err(_) => answered.recv().ok(),
                        }
                    }
                    // The app has ended without an answer; the run sees the channel close.
                    Err(RecvTimeoutError::Disconnected) => None,
                };
                if let Some(answer) = answer
                    && let Err(error) = connection.send(&answer)
                {
                    tracing::warn!("ответ DevCast не отправлен: {error}");
                }
                drop(written);
            });
        }
    });
    Calls(receiver)
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

    #[test]
    fn a_request_the_window_does_not_take_is_refused_and_not_done_later() {
        let name = format!("devcast-test-busy-{}", std::process::id());
        let listener = Listener::bind(&name).unwrap();
        let pickup = Duration::from_millis(100);
        let calls = serve_within(listener, egui::Context::default(), pickup);
        // The window is busy (a system dialog): nobody takes the request.
        let answer: Answer = dc_ipc::request(&name, &Request::Show).unwrap();
        assert_eq!(answer, Answer::Refused(BUSY.to_owned()));
        assert_eq!(
            calls.try_iter().count(),
            0,
            "the run gave up: not done later"
        );

        // Taken in time, the answer may take longer than the pickup.
        let slow = std::thread::spawn(move || {
            let call = calls.recv().unwrap();
            std::thread::sleep(pickup * 3);
            call.reply.send(Answer::Shown);
        });
        let answer: Answer = dc_ipc::request(&name, &Request::Show).unwrap();
        assert_eq!(answer, Answer::Shown);
        slow.join().unwrap();
    }
}
