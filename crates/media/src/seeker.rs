//! Decoding on its own thread for the editor: every seek gets the next generation number, the
//! thread works only on the newest request (older ones still waiting are skipped) and every
//! answer carries the generation it was made for. A frame of an older generation is stale (the
//! pointer has moved on) and is dropped, so a slow decode never shows an old frame over a new one.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use dc_core::SourceTime;

use crate::decoder::{Decoder, Frame};
use crate::error::MediaError;
use crate::pool::FramePool;
use crate::probe::Probe;

/// How exact a seek must be (decision 003): while the pointer is dragged the nearest keyframe,
/// on release the exact frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Keyframe,
    Exact,
}

struct Request {
    generation: u64,
    time: SourceTime,
    precision: Precision,
}

type Answer = (u64, Result<Frame, MediaError>);

pub struct Seeker {
    path: PathBuf,
    requests: Sender<Request>,
    answers: Receiver<Answer>,
    generation: u64,
}

impl Seeker {
    /// Starts the decoding thread; the decoder is opened there (FFmpeg's scaler cannot move
    /// between threads), so an error opening it comes as the answer to the first seek.
    pub fn start(path: &Path, probe: Probe, pool: FramePool) -> Self {
        let (requests, inbox) = channel::<Request>();
        let (outbox, answers) = channel::<Answer>();
        let file = path.to_path_buf();
        std::thread::spawn(move || {
            let mut decoder = match Decoder::open(&file, probe, pool) {
                Ok(decoder) => decoder,
                Err(error) => {
                    if let Some(request) = newest(&inbox) {
                        // Nobody left to tell if the seeker is gone already.
                        let _ = outbox.send((request.generation, Err(error)));
                    }
                    return;
                }
            };
            while let Some(request) = newest(&inbox) {
                let frame = match request.precision {
                    Precision::Exact => decoder.frame_at(request.time),
                    Precision::Keyframe => decoder.keyframe_at(request.time),
                };
                if outbox.send((request.generation, frame)).is_err() {
                    return;
                }
            }
        });
        Self {
            path: path.to_path_buf(),
            requests,
            answers,
            generation: 0,
        }
    }

    /// Asks for the frame at `time`; returns the generation of this request.
    pub fn seek(&mut self, time: SourceTime, precision: Precision) -> Result<u64, MediaError> {
        self.generation += 1;
        let request = Request {
            generation: self.generation,
            time,
            precision,
        };
        self.requests.send(request).map_err(|_| self.stopped())?;
        Ok(self.generation)
    }

    /// The answer to the newest seek if it is ready; stale answers are dropped.
    pub fn poll(&mut self) -> Result<Option<Frame>, MediaError> {
        loop {
            match self.answers.try_recv() {
                Ok((generation, answer)) if generation == self.generation => {
                    return answer.map(Some);
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => return Err(self.stopped()),
            }
        }
    }

    /// Waits for the answer to the newest seek; stale answers are dropped.
    pub fn wait(&mut self) -> Result<Frame, MediaError> {
        loop {
            let (generation, answer) = self.answers.recv().map_err(|_| self.stopped())?;
            if generation == self.generation {
                return answer;
            }
        }
    }

    fn stopped(&self) -> MediaError {
        MediaError::Stopped {
            path: self.path.clone(),
        }
    }
}

/// Waits for a request and skips to the newest one waiting; `None` once the seeker is dropped.
fn newest(inbox: &Receiver<Request>) -> Option<Request> {
    let mut request = inbox.recv().ok()?;
    while let Ok(newer) = inbox.try_recv() {
        request = newer;
    }
    Some(request)
}
