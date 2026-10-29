//! Messages between devcast processes and the editor over a local socket (decision 011): a
//! named pipe on Windows, a socket file elsewhere, its name per user. A message is JSON with
//! its length (u32, little-endian) in front; a connection carries one request and its answer.

use std::io::{Read, Write};

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Name, Stream};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Larger messages are refused: a broken or foreign peer must not make us allocate gigabytes.
pub const MAX_MESSAGE: u32 = 16 << 20;

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Nobody listens on the name: the process is not running.
    #[error("никто не слушает канал «{0}»")]
    NotRunning(String),
    #[error("канал «{name}» занят: {source}")]
    Taken {
        name: String,
        source: std::io::Error,
    },
    #[error("связь по каналу оборвалась: {0}")]
    Io(#[from] std::io::Error),
    #[error("сообщение не прочитано: {0}")]
    BadMessage(String),
    #[error("не задано имя пользователя (переменная USERNAME): запусти devcast из обычного сеанса")]
    NoUser,
}

/// Socket name of `service` (`record`, `editor` …) for the current user, so that two users
/// on one machine do not meet.
pub fn socket_name(service: &str) -> Result<String, IpcError> {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .map_err(|_| IpcError::NoUser)?;
    Ok(format!("devcast-{service}-{user}"))
}

/// Sends `request` to the process listening on `name` and waits for its answer.
pub fn request<Q: Serialize, A: DeserializeOwned>(name: &str, request: &Q) -> Result<A, IpcError> {
    let mut stream = match Stream::connect(ns_name(name)?) {
        Ok(stream) => stream,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(IpcError::NotRunning(name.to_owned()));
        }
        Err(e) => return Err(IpcError::Io(e)),
    };
    send(&mut stream, request)?;
    receive(&mut stream)
}

/// Accepts connections on a name; only one listener per name can exist.
pub struct Listener {
    name: String,
    listener: interprocess::local_socket::Listener,
}

/// One accepted connection: read the request, then send the answer.
pub struct Connection(Stream);

impl Listener {
    pub fn bind(name: &str) -> Result<Self, IpcError> {
        let listener = ListenerOptions::new()
            .name(ns_name(name)?)
            .create_sync()
            .map_err(|source| IpcError::Taken {
                name: name.to_owned(),
                source,
            })?;
        Ok(Self {
            name: name.to_owned(),
            listener,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Waits for the next connection.
    pub fn accept(&self) -> Result<Connection, IpcError> {
        Ok(Connection(self.listener.accept()?))
    }

    /// Wakes the thread that waits in [`Self::accept`] or comes back to it, so that it can see
    /// it is time to end: connects and stays until the listener drops the connection. A client
    /// that hangs up before it is accepted is skipped by `accept` (interprocess), so hanging up
    /// at once would leave that thread waiting forever. Nobody listening — nothing to wake.
    pub fn wake(name: &str) -> Result<(), IpcError> {
        let mut stream = match Stream::connect(ns_name(name)?) {
            Ok(stream) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(IpcError::Io(e)),
        };
        // The listener sends nothing: the read ends when it drops the connection.
        let _ = stream.read(&mut [0; 1]);
        Ok(())
    }
}

impl Connection {
    pub fn receive<T: DeserializeOwned>(&mut self) -> Result<T, IpcError> {
        receive(&mut self.0)
    }

    pub fn send<T: Serialize>(&mut self, message: &T) -> Result<(), IpcError> {
        send(&mut self.0, message)
    }
}

fn ns_name(name: &str) -> Result<Name<'_>, IpcError> {
    Ok(name.to_ns_name::<GenericNamespaced>()?)
}

/// Writes one message: length, then JSON.
pub fn send<T: Serialize>(stream: &mut impl Write, message: &T) -> Result<(), IpcError> {
    let json = serde_json::to_vec(message).map_err(|e| IpcError::BadMessage(e.to_string()))?;
    let length = u32::try_from(json.len())
        .ok()
        .filter(|l| *l <= MAX_MESSAGE)
        .ok_or_else(|| IpcError::BadMessage(format!("{} байт — слишком много", json.len())))?;
    stream.write_all(&length.to_le_bytes())?;
    stream.write_all(&json)?;
    stream.flush()?;
    Ok(())
}

/// Reads one message written by [`send`].
pub fn receive<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T, IpcError> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length);
    if length > MAX_MESSAGE {
        return Err(IpcError::BadMessage(format!(
            "{length} байт — слишком много"
        )));
    }
    let mut json = vec![0; length as usize];
    stream.read_exact(&mut json)?;
    serde_json::from_slice(&json).map_err(|e| IpcError::BadMessage(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_name(case: &str) -> String {
        socket_name(&format!("test-{case}-{}", std::process::id())).unwrap()
    }

    #[test]
    fn messages_round_trip_with_their_length() {
        let mut bytes = Vec::new();
        send(&mut bytes, &vec!["пауза", "стоп"]).unwrap();
        assert_eq!(bytes[..4], 25u32.to_le_bytes());
        let read: Vec<String> = receive(&mut bytes.as_slice()).unwrap();
        assert_eq!(read, ["пауза", "стоп"]);
        let mut huge = (MAX_MESSAGE + 1).to_le_bytes().to_vec();
        huge.extend_from_slice(b"{}");
        let error = receive::<Vec<String>>(&mut huge.as_slice()).unwrap_err();
        assert!(matches!(error, IpcError::BadMessage(_)), "{error}");
        let error = receive::<Vec<String>>(&mut &b"\x02\x00"[..]).unwrap_err();
        assert!(matches!(error, IpcError::Io(_)), "{error}");
    }

    #[test]
    fn a_request_gets_the_answer_of_the_listener() {
        let name = test_name("answer");
        let listener = Listener::bind(&name).unwrap();
        assert!(matches!(Listener::bind(&name), Err(IpcError::Taken { .. })));
        let server = std::thread::spawn(move || {
            let mut connection = listener.accept().unwrap();
            let question: String = connection.receive().unwrap();
            connection.send(&format!("{question}: готово")).unwrap();
        });
        let answer: String = request(&name, &"стоп").unwrap();
        assert_eq!(answer, "стоп: готово");
        server.join().unwrap();
    }

    /// The wake comes right after an answer, while the listener is not yet back in `accept`:
    /// a wake that hung up at once was skipped there (interprocess drops clients that left
    /// before being accepted), and the listener thread waited forever.
    #[test]
    fn wake_reaches_a_listener_that_is_still_answering() {
        for round in 0..200 {
            let name = test_name(&format!("wake-{round}"));
            let listener = Listener::bind(&name).unwrap();
            let closing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = std::sync::Arc::clone(&closing);
            let (ended, end) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                loop {
                    let connection = listener.accept();
                    if flag.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    let mut connection = connection.unwrap();
                    let question: String = connection.receive().unwrap();
                    connection.send(&question).unwrap();
                }
                ended.send(()).unwrap();
            });
            let answer: String = request(&name, &"стоп").unwrap();
            assert_eq!(answer, "стоп");
            closing.store(true, std::sync::atomic::Ordering::SeqCst);
            Listener::wake(&name).unwrap();
            let waited = end.recv_timeout(std::time::Duration::from_secs(10));
            assert!(
                waited.is_ok(),
                "round {round}: the listener did not wake up"
            );
        }
    }

    #[test]
    fn nobody_listening_is_not_running() {
        let name = test_name("nobody");
        let error = request::<_, String>(&name, &"стоп").unwrap_err();
        assert!(matches!(error, IpcError::NotRunning(_)), "{error}");
    }
}
