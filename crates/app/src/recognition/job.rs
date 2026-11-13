//! The editor's side of a recognition: the worker process, the thread `devcast-recognition`
//! that hands it the request and reads its answer lines, and the kill that cancels it. The UI
//! never waits: the thread wakes the main window after every line (app CLAUDE.md), and the
//! editor takes the messages in its frame.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

use dc_ops::recognition::WORKER_ARG;
use dc_ui_kit::strings::captions as text;

use super::{Message, Request};

/// Starts a worker process with its stdin and stdout piped.
pub type Spawn = Box<dyn Fn() -> std::io::Result<Child> + Send>;

/// `devcast --captions-worker`: the CLI exe beside this one is the worker of the app.
pub fn cli_worker() -> Spawn {
    Box::new(|| {
        let mut worker = Command::new(beside(&std::env::current_exe()?)?);
        worker
            .arg(WORKER_ARG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // The worker is a console program: without the flag Windows opens a console window
        // over the editor.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            worker.creation_flags(CREATE_NO_WINDOW);
        }
        worker.spawn()
    })
}

/// A running recognition; dropping it kills the worker.
#[derive(Debug)]
pub struct Job {
    child: Child,
    answers: Receiver<Message>,
    /// The reader thread; the editor never waits for it, tests do (`settle`).
    #[cfg(test)]
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Job {
    /// Hands `request` to the started `child` and reads its answer on a thread that calls
    /// `wake` after every message.
    pub fn start(
        mut child: Child,
        request: &Request,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let json = serde_json::to_string(request).map_err(|e| text::failed_start(&e))?;
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(text::failed_start(&"нет каналов процесса"));
        };
        let (sender, answers) = channel();
        #[cfg_attr(not(test), expect(unused_mut, reason = "only tests keep the reader"))]
        let mut job = Self {
            child,
            answers,
            #[cfg(test)]
            reader: None,
        };
        let read = move || {
            hand(input, &json);
            listen(BufReader::new(output), &sender, &wake);
        };
        // A thread that cannot start drops the job: the worker is killed.
        let reader = std::thread::Builder::new()
            .name("devcast-recognition".to_owned())
            .spawn(read)
            .map_err(|e| text::failed_start(&e))?;
        #[cfg(test)]
        {
            job.reader = Some(reader);
        }
        #[cfg(not(test))]
        drop(reader);
        Ok(job)
    }

    /// The messages that came since the last call.
    pub fn receive(&self) -> Vec<Message> {
        self.answers.try_iter().collect()
    }

    /// Waits for the next message (tests): `None` once the reader is gone.
    #[cfg(test)]
    pub fn next(&self) -> Option<Message> {
        self.answers.recv().ok()
    }

    /// Waits until the whole answer is read (tests): `receive` then gets all of it.
    #[cfg(test)]
    pub fn settle(&mut self) {
        if let Some(reader) = self.reader.take() {
            assert!(reader.join().is_ok(), "the reader of the answer panicked");
        }
    }

    /// Kills the worker and waits for it to end: how it ended (tests; the editor drops the job).
    #[cfg(test)]
    pub fn cancel(mut self) -> std::io::Result<std::process::ExitStatus> {
        self.stop();
        self.child.wait()
    }

    fn stop(&mut self) {
        if let Ok(Some(_)) = self.child.try_wait() {
            return;
        }
        if let Err(error) = self.child.kill() {
            tracing::warn!("процесс распознавания не остановлен: {error}");
        }
        if let Err(error) = self.child.wait() {
            tracing::warn!("процесс распознавания не дождались: {error}");
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Writes the request and closes the worker's stdin. A worker that ended before reading it
/// says why on its stdout, or the reader reports the missing answer.
fn hand(mut input: impl Write, json: &str) {
    if let Err(error) = input.write_all(json.as_bytes()) {
        tracing::warn!("запрос не передан процессу распознавания: {error}");
    }
}

/// Sends the messages of `output` until the last one; an answer that ends without it is a
/// failure. Stops when nobody listens any more (the job was cancelled).
fn listen(output: impl BufRead, sender: &Sender<Message>, wake: &dyn Fn()) {
    for line in output.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                tracing::warn!("ответ процесса распознавания не прочитан: {error}");
                break;
            }
        };
        let Some(message) = Message::parse(&line) else {
            tracing::warn!("непонятная строка процесса распознавания: {line}");
            continue;
        };
        let last = message.is_last();
        if sender.send(message).is_err() {
            return;
        }
        wake();
        if last {
            return;
        }
    }
    if sender
        .send(Message::Failed(text::NO_ANSWER.to_owned()))
        .is_ok()
    {
        wake();
    }
}

/// The CLI exe `devcast` in the folder of `exe`: the installer ships them side by side, and
/// cargo builds them into the same `target/<profile>` (docs/decisions/018-recognition-process.md).
pub fn beside(exe: &Path) -> std::io::Result<PathBuf> {
    let path = exe.with_file_name(format!("devcast{}", std::env::consts::EXE_SUFFIX));
    if path.is_file() {
        Ok(path)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            text::no_worker(&path),
        ))
    }
}
