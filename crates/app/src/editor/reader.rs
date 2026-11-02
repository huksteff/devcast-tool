//! Reading files of the project off the UI thread: the peaks of the waves (`waves.rs`, 7.11)
//! and the clicks of the recordings (`presses.rs`, 7.15). The UI asks for what it wants and never
//! waits: its thread reads one source at a time, wakes the main window after every answer (app
//! CLAUDE.md), and the main window wakes the editor (`take_news`). The window keeps every source
//! it read for as long as it is open; an unreadable one stays empty with a warning in the log.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

/// What a reader reads: its thread and the warning of a failed read.
pub trait Wanted: Clone + PartialEq + Send + std::fmt::Debug + 'static {
    /// The name of the reading thread.
    const THREAD: &'static str;
    /// The warning of the log when reading failed (the reason follows).
    const FAILED: &'static str;
}

#[derive(Debug)]
enum Slot<V> {
    Waiting,
    Ready(Arc<V>),
    Failed,
}

#[derive(Debug)]
pub struct Reader<K, V> {
    slots: Vec<(K, Slot<V>)>,
    worker: Option<Worker<K, V>>,
}

impl<K, V> Default for Reader<K, V> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            worker: None,
        }
    }
}

#[derive(Debug)]
struct Worker<K, V> {
    requests: Sender<K>,
    answers: Receiver<(K, Result<V, String>)>,
    news: Arc<AtomicBool>,
    /// Last: joined after the channels close (`Joined`).
    _thread: super::threads::Joined,
}

impl<K: Wanted, V: Send + std::fmt::Debug + 'static> Reader<K, V> {
    /// Starts the thread that reads with `load`; it calls `wake` after every answer.
    pub fn start_with(
        &mut self,
        load: impl Fn(&K) -> Result<V, String> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> std::io::Result<()> {
        let (requests, inbox) = channel::<K>();
        let (outbox, answers) = channel::<(K, Result<V, String>)>();
        let news = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&news);
        let thread = std::thread::Builder::new()
            .name(K::THREAD.to_owned())
            .spawn(move || {
                for source in inbox {
                    let read = load(&source);
                    if outbox.send((source, read)).is_err() {
                        return;
                    }
                    flag.store(true, Ordering::Release);
                    wake();
                }
            })?;
        let worker = Worker {
            requests,
            answers,
            news,
            _thread: super::threads::Joined::new(thread),
        };
        for (source, _) in self
            .slots
            .iter()
            .filter(|(_, s)| matches!(s, Slot::Waiting))
        {
            // A send fails only when the thread is gone: those sources stay empty.
            let _ = worker.requests.send(source.clone());
        }
        self.worker = Some(worker);
        Ok(())
    }

    /// Asks for the sources of `wanted` it does not have yet.
    pub fn want<'a>(&mut self, wanted: impl Iterator<Item = &'a K>) {
        for source in wanted {
            if self.slots.iter().any(|(s, _)| s == source) {
                continue;
            }
            self.slots.push((source.clone(), Slot::Waiting));
            if let Some(worker) = &self.worker
                && worker.requests.send(source.clone()).is_err()
            {
                self.arrive(
                    source.clone(),
                    Err(format!("поток {} остановлен", K::THREAD)),
                );
            }
        }
    }

    /// Takes the answers of the thread; whether any came.
    pub fn receive(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let answers: Vec<_> = worker.answers.try_iter().collect();
        let any = !answers.is_empty();
        for (source, read) in answers {
            self.arrive(source, read);
        }
        any
    }

    /// What `source` holds came (or failed).
    pub fn arrive(&mut self, source: K, read: Result<V, String>) {
        let slot = match read {
            Ok(value) => Slot::Ready(Arc::new(value)),
            Err(error) => {
                tracing::warn!("{}: {error}", K::FAILED);
                Slot::Failed
            }
        };
        match self.slots.iter_mut().find(|(s, _)| *s == source) {
            Some((_, old)) => *old = slot,
            None => self.slots.push((source, slot)),
        }
    }

    /// What `source` holds, once it came.
    pub fn get(&self, source: &K) -> Option<&V> {
        self.slots.iter().find_map(|(s, slot)| match slot {
            Slot::Ready(value) if s == source => Some(value.as_ref()),
            _ => None,
        })
    }

    /// The thread answered since the last call: the editor window repaints.
    pub fn take_news(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|w| w.news.swap(false, Ordering::AcqRel))
    }
}
