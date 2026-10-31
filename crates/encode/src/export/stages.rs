//! Where the time of an export went (task 5.41): busy time of every stage, summed over frames.

use std::time::{Duration, Instant};

/// Busy time of the stages of one export, all on the export thread: their sum is about the
/// wall time. The GPU and a hardware codec work alongside it, so their share shows up as
/// waiting: in `gpu_wait`, and for a hardware codec in `encode` (it keeps a few frames in
/// flight and blocks when they are all busy).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stages {
    /// Opening the codecs and the file, before the first frame.
    pub open: Duration,
    /// The scene of every moment (`Timeline::scene`).
    pub scene: Duration,
    /// Decoding video frames and pictures and uploading them to the GPU.
    pub decode: Duration,
    /// Recording and submitting the draw.
    pub draw: Duration,
    /// Converting a frame (NV12 for MP4; RGBA for GIF) and waiting for it in memory.
    pub gpu_wait: Duration,
    /// Copying a frame from the GPU buffer into the codec's frame.
    pub copy: Duration,
    /// Mixing the sound.
    pub sound: Duration,
    /// The video codec (GIF: gifski taking the frame).
    pub encode: Duration,
    /// Encoding the sound, writing packets into the file and closing it.
    pub mux: Duration,
}

impl Stages {
    /// The stages with their names, as the CLI prints them.
    pub fn named(&self) -> [(&'static str, Duration); 9] {
        [
            ("open", self.open),
            ("scene", self.scene),
            ("decode", self.decode),
            ("draw", self.draw),
            ("gpu_wait", self.gpu_wait),
            ("copy", self.copy),
            ("sound", self.sound),
            ("encode", self.encode),
            ("mux", self.mux),
        ]
    }
}

/// Runs `work` and adds its time to `stage`.
pub(crate) fn timed<T>(stage: &mut Duration, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = work();
    *stage += start.elapsed();
    result
}
