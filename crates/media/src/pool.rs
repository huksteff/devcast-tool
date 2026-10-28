//! Memory for decoded frames with a fixed limit. A frame's buffer goes back to the pool when the
//! frame is dropped and is reused for the next frame of the same size; the pool never holds more
//! than its limit, so a consumer that keeps every frame gets an error instead of eating memory.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::error::MediaError;

#[derive(Debug)]
struct State {
    limit: usize,
    /// Bytes of every buffer the pool owns: handed out and free.
    allocated: usize,
    peak: usize,
    free: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct FramePool {
    state: Arc<Mutex<State>>,
}

/// A buffer of the pool; returns to it on drop.
#[derive(Debug)]
pub struct Buffer {
    data: Vec<u8>,
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    // Nothing under the lock can panic, so a poisoned lock still guards consistent counters.
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FramePool {
    pub fn new(limit_bytes: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                limit: limit_bytes,
                allocated: 0,
                peak: 0,
                free: Vec::new(),
            })),
        }
    }

    /// A buffer of `len` bytes: a free one of that size, or a new one if the limit allows
    /// (free buffers of other sizes are released first).
    pub fn take(&self, len: usize) -> Result<Buffer, MediaError> {
        let mut state = lock(&self.state);
        let data = match state.free.iter().position(|b| b.len() == len) {
            Some(index) => state.free.swap_remove(index),
            None => {
                while state.allocated + len > state.limit {
                    let Some(released) = state.free.pop() else {
                        return Err(MediaError::PoolFull {
                            limit_mb: state.limit / 1_000_000,
                        });
                    };
                    state.allocated -= released.len();
                }
                state.allocated += len;
                state.peak = state.peak.max(state.allocated);
                vec![0; len]
            }
        };
        Ok(Buffer {
            data,
            state: Arc::clone(&self.state),
        })
    }

    /// Bytes the pool owns now.
    pub fn allocated(&self) -> usize {
        lock(&self.state).allocated
    }

    /// The most bytes the pool has owned at once.
    pub fn peak(&self) -> usize {
        lock(&self.state).peak
    }
}

impl Buffer {
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let data = std::mem::take(&mut self.data);
        lock(&self.state).free.push(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_are_reused_and_the_limit_holds() {
        let pool = FramePool::new(100);
        let a = pool.take(40).unwrap();
        let b = pool.take(40).unwrap();
        let error = pool.take(40).unwrap_err();
        assert!(matches!(error, MediaError::PoolFull { .. }), "{error}");
        drop(a);
        let c = pool.take(40).unwrap();
        assert_eq!(pool.allocated(), 80);
        drop((b, c));
        let big = pool.take(90).unwrap();
        assert_eq!(big.as_slice().len(), 90);
        assert_eq!(pool.allocated(), 90);
        assert_eq!(pool.peak(), 90);
    }
}
